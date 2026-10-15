package main

import (
	"encoding/json"
	"flag"
	"log"
	"os"
	"unicode/utf8"

	"golang.org/x/text/cases"
)

func main() {
	output := flag.String("output", "", "path to generated case-fold map")
	flag.Parse()
	if *output == "" {
		log.Fatal("-output is required")
	}
	fold := cases.Fold()
	mapping := make(map[string]string)
	for character := rune(0); character <= utf8.MaxRune; character++ {
		if character >= 0xd800 && character <= 0xdfff {
			continue
		}
		original := string(character)
		folded := fold.String(original)
		if folded != original {
			mapping[original] = folded
		}
	}
	data, err := json.MarshalIndent(mapping, "", "  ")
	if err != nil {
		log.Fatal(err)
	}
	data = append(data, '\n')
	if err := os.WriteFile(*output, data, 0o644); err != nil {
		log.Fatal(err)
	}
}
