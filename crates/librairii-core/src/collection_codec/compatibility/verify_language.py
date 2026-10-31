#!/usr/bin/env python3
"""Compare actual frozen Default parsing with a separately owned DNS-free fork.

This is qualification glue. The counterfactual writes ONLY a new independent
authenticated module copy: mask regionTypes bit1/value2; remove both TLD APIs.
Original inputs and all unmodified members are checked before/after execution.
"""
import hashlib
import json
from pathlib import Path
import runpy
import shutil


def main():
    directory = Path(__file__).resolve().parent
    helper = runpy.run_path(str(directory / "verify_normalization.py"))
    command, inventory, digest = (helper[name] for name in ["command", "inventory", "sha"])
    parser = runpy.run_path(str(directory / "extract_normalization.py"))
    seed = directory / "language-cases.json"
    seed_bytes = seed.read_bytes()

    def counterfactual(inputs):
        original = json.loads(command([str(inputs["binary"]), str(seed)], inputs["checkout"], inputs["environment"]))
        masked = inputs["temporary"] / "xtext-without-dns"
        shutil.copytree(inputs["module"], masked)
        if inventory(masked) != inputs["module_inventory"]:
            raise ValueError("independent counterfactual copy differs")
        table = masked / "internal/language/tables.go"
        source = table.read_text(encoding="utf-8")
        values = parser["numbers"](source, "regionTypes", 359)
        marker = "var regionTypes = [359]uint8{"
        start = source.index(marker) + len(marker)
        end = source.index("}", start)
        replacement = "\n" + "".join(f"\t{value & ~2},\n" for value in values)
        table.write_text(source[:start] + replacement + source[end:], encoding="utf-8")

        def remove_method(path):
            source = path.read_text(encoding="utf-8")
            signature = "func (r Region) TLD() (Region, error) {"
            start = source.index(signature)
            if source.count(signature) != 1:
                raise ValueError("changed selected TLD API shape")
            end, depth = start + len(signature), 1
            while depth:
                depth += (source[end] == "{") - (source[end] == "}")
                end += 1
            path.write_text(source[:start] + source[end:], encoding="utf-8")

        remove_method(masked / "internal/language/language.go")
        remove_method(masked / "language/language.go")
        masked_before = inventory(masked)
        changed = {name for name, sha in masked_before.items() if sha != inputs["module_inventory"][name]}
        expected = {"internal/language/tables.go", "internal/language/language.go", "language/language.go"}
        if changed != expected or set(masked_before) != set(inputs["module_inventory"]):
            raise ValueError("counterfactual widened beyond exact DNS subset")
        modfile = inputs["temporary"] / "without-dns.mod"
        content = inputs["modfile"].read_text(encoding="utf-8").replace(str(inputs["module"]), str(masked))
        modfile.write_text(content, encoding="utf-8")
        binary = inputs["temporary"] / "language-without-dns-proof"
        command([str(inputs["go_tool"]), "build", "-mod=mod", "-modfile=" + str(modfile),
                 "-o", str(binary), "./cmd/native-normalization-proof"], inputs["checkout"], inputs["environment"])
        result = json.loads(command([str(binary), str(seed)], inputs["checkout"], inputs["environment"]))
        if result != original:
            raise ValueError("DNS-free counterfactual changed Default/String/Und/error or product-boundary outcomes")
        if inventory(masked) != masked_before or seed.read_bytes() != seed_bytes:
            raise ValueError("qualification changed counterfactual/seed inputs")
        original["dnsOmission"] = {
            "equalRawAndProductResults": True,
            "removed": ["regionTypes.ccTLD-bit1-value2", "internal/language.Region.TLD", "language.Region.TLD"],
            "unchangedMembers": len(masked_before) - len(changed),
            "changedMembers": [{"path": name, "originalSha256": inputs["module_inventory"][name],
                                "dnsFreeSha256": masked_before[name]} for name in sorted(changed)],
            "regionFlags": {"count": len(values), "retainedOtherBits": True,
                            "maskedByteSha256": digest(bytes(value & ~2 for value in values))},
        }
        return original

    result = helper["qualify"]("language_oracle.go.in", (str(seed),), counterfactual)
    result["seedSha256"] = digest(seed_bytes)
    result["sourceCopyright"] = ["// Copyright 2013 The Go Authors. All rights reserved.",
                                 "// Copyright 2015 The Go Authors. All rights reserved.",
                                 "// Copyright 2018 The Go Authors. All rights reserved."]
    output = directory / "language-reference.json"
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps({"referenceSha256": hashlib.sha256(output.read_bytes()).hexdigest(),
                      "groups": {key: value["count"] for key, value in result["groups"].items()},
                      "dnsOmission": result["dnsOmission"]}))


if __name__ == "__main__":
    main()
