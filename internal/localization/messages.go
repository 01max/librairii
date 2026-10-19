package localization

import (
	"embed"
	"encoding/json"
	"fmt"
	"strings"
)

//go:embed locales/*.json
var catalogs embed.FS
var messages = load()

func load() map[string]map[string]string {
	result := map[string]map[string]string{}
	for _, lang := range []string{"en", "fr"} {
		data, _ := catalogs.ReadFile("locales/" + lang + ".json")
		var values map[string]string
		if err := json.Unmarshal(data, &values); err != nil {
			panic(err)
		}
		result[lang] = values
	}
	return result
}
func Text(language, key string, values ...any) string {
	text := messages[language][key]
	if text == "" {
		text = messages["en"][key]
	}
	if text == "" {
		text = key
	}
	for i, value := range values {
		text = strings.ReplaceAll(text, fmt.Sprintf("{%d}", i), fmt.Sprint(value))
	}
	return text
}
