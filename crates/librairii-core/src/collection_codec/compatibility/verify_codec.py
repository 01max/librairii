#!/usr/bin/env python3
"""Qualify saved wire semantics using actual Go, never the preservation corpus.

Only the unchanged query.go is additionally compiled as its own internal
package; this avoids pulling a SQLite driver into a PURE codec experiment.
Every original436 file and487 authenticated module member is checked unchanged.
"""
import json
from pathlib import Path
import random
from verify_normalization import command, qualify, sha


def inputs():
    cases = []

    def add(name, raw, versions=range(1, 7), legacy=(0, 7)):
        raw = raw.encode() if isinstance(raw, str) else raw
        for version in versions:
            for identifier in legacy:
                cases.append(dict(name=name, bytes=raw.hex(), version=version, legacyId=identifier))

    selected = ["{}", "null", " null ", "[]", "0", "{} {}", "{", "{\"name\":true}",
                '{"name":"Moon","Name":""}', '{"name":"Moon","name":null}',
                '{"name":"Moon","unknown":null}', '{"name":null}',
                '{"languages":null}', '{"languages":[null]}',
                '{"booleanFilters":[null]}', '{"choiceFilters":[null]}',
                '{"anyOf":[null]}', '{"anyOf":null}', '{"anyOf":[{"page":1}]}',
                '{"includeBroken":true}', '{"brokenVisibility":"hide"}',
                '{"brokenState":"ignored"}', '{"brokenState":"invalid"}',
                '{"booleanFilters":[{"definitionId":7,"state":"true"}]}',
                '{"booleanFilters":[{"definitionId":7,"state":"ignored"}]}',
                '{"booleanFilters":[{"definitionId":7,"state":"true"},{"definitionId":7,"state":"false"}]}',
                '{"booleanFilters":[{"definitionId":7,"state":"ignored"}],"choiceFilters":[{"definitionId":7,"valueIds":[2,2]}]}',
                '{"anyOf":[{"booleanFilters":[{"definitionId":7,"state":"false"}]},{"name":"𝑨"}]}',
                '{"name":"𝑨"}', '{"name":"<>&\\u2028X\\u2029"}',
                '{"name":"a' + '\U00016ff0' * 29 + '\u1715\U00016ff0Z"}',
                '{"name":"\\ud800X\\udc00"}', '{"name":"\\ud800\\udc00"}',
                '{"name":"\\ud800\\u0041"}', '{"name":"\\ud800\\uqqqq"}',
                '{"name":"\\ud800\\x"}', '{"name":"\\udc00\\ud800"}',
                '{"languageſ":["en"],"languages":null}',
                '{"booleanFilters":[{"definitionId":1,"ſtate":"true","state":null}]}',
                '{"booleanFilters":[{"definitionId":1,"state":"false","ſtate":"true"}]}',
                '{"booleanﬁlters":[]}', '{"NAME":"Hi"}',
                '{"INCLUDEBROKEN":false}', '{"brokenVisibility":"only","anyOf":[{"includeBroken":true}]}',
                '{"name":"  L’ÉCOLE  ","languages":["fr_fr","en-GB","fr-FR"],"compatibilities":["missing","compatible","missing"],"choiceFilters":[{"definitionId":5,"valueIds":[7,2,7]}]}']
    for n, raw in enumerate(selected):
        add("selected-" + str(n), raw)
    for n, raw in enumerate([b'{"name":"\xe2\x82"}', b'{"name":"\xed\xa0\x80"}', b'{"name":"\xc0\xaf"}',
                              b'{"name":"\xf4\x90\x80\x80"}', b'{"name":"\xff\x00"}', b'{"\xff":null}']):
        add("raw-utf8-" + str(n), raw)
    for raw in ["-0", "0", "1", "-1", "1.0", "1e0", "1E+0", "01", "+1", "-", "1e309",
                "9223372036854775807", "9223372036854775808", "-9223372036854775808", "-9223372036854775809"]:
        add("integer-" + raw, '{"page":' + raw + '}')
        add("definition-" + raw, '{"booleanFilters":[{"definitionId":' + raw + ',"state":"true"}]}')
    for field in ["name", "languages", "compatibilities", "booleanFilters", "choiceFilters", "anyOf", "page", "pageSize", "sort", "brokenVisibility", "brokenState", "includeBroken"]:
        for value in [None, False, 0, "", [], {}]:
            add("field-type-" + field + "-" + repr(value), json.dumps({field: value}, separators=(",", ":")))
    for nodes in [1, 49, 50, 51]:
        add("flat-nodes-" + str(nodes), json.dumps({"anyOf": [None] * (nodes - 1)}))
        raw = "{}"
        for _ in range(nodes - 1):
            raw = '{"anyOf":[' + raw + ']}'
        add("deep-nodes-" + str(nodes), raw)
    for n in [100, 101]:
        for field in ["languages", "compatibilities"]:
            add("raw-array-" + field + "-" + str(n), json.dumps({field: ["en" if field == "languages" else "missing"] * n}))
        add("raw-values-" + str(n), json.dumps({"choiceFilters": [dict(definitionId=1, valueIds=[2] * n)]}))
    for n in [49, 50, 51]:
        add("filter-groups-" + str(n), json.dumps({"booleanFilters": [dict(definitionId=i + 1, state="true") for i in range(n)]}))
    for n in [200, 201]:
        add("name-runes-" + str(n), json.dumps({"name": "X" * n}))
    # Deterministic structured adversaries retain raw counts, flags, shared IDs
    # and branch order; random choices are inputs, never expected outcomes.
    randomizer = random.Random(0x7155ac31)
    names = ["ÉTÉ", "𝑨", "İ", "ß", "Moon", "<>&", "", "\u034f"]
    languages = ["fr_fr", "en-GB", "und", "sh", "mo", "en-u-rg-uszzzz", "en-BOGUS", ""]
    for index in range(400):
        q = dict(name=randomizer.choice(names), languages=[randomizer.choice(languages) for _ in range(randomizer.randrange(4))],
                 booleanFilters=[dict(definitionId=randomizer.randrange(0, 10), state=randomizer.choice(["true", "false", "ignored", "maybe"])) for _ in range(randomizer.randrange(4))],
                 choiceFilters=[dict(definitionId=randomizer.randrange(0, 10), valueIds=[randomizer.randrange(0, 6) for _ in range(randomizer.randrange(4))]) for _ in range(randomizer.randrange(3))])
        if index % 3 == 0:
            q["anyOf"] = [dict(name=randomizer.choice(names)), dict(includeBroken=index % 2 == 0)]
        if index % 4 == 0:
            q["brokenState"] = randomizer.choice(["", "ignored", "true", "invalid"])
        add("structured-" + str(index), json.dumps(q, ensure_ascii=False, separators=(",", ":")),
            versions=(randomizer.randrange(1, 6),), legacy=(0, 7))
    return cases


def main():
    cases = inputs()
    def run(context):
        path = context["temporary"] / "codec-inputs.json"
        encoded = json.dumps(cases, separators=(",", ":"), sort_keys=True).encode()
        path.write_bytes(encoded)
        output = json.loads(command([str(context["binary"]), str(path)], context["checkout"], context["environment"], timeout=120))
        output["inputSha256"] = sha(encoded)
        output["recipeSha256"] = sha(Path(__file__).read_bytes())
        return output
    result = qualify("codec_oracle.go.in", runner=run,
                     copies=(("internal/collections/query.go", "internal/nativecodecproof/query.go"),))
    output = Path(__file__).resolve().parent / "codec-reference.json"
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"cases": len(cases), "sha256": sha(output.read_bytes()),
                      "decodeAccepted": sum(c["decodeError"] == "" for c in result["cases"]),
                      "migrateAccepted": sum(c["migrateError"] == "" for c in result["cases"])}))


if __name__ == "__main__":
    main()
