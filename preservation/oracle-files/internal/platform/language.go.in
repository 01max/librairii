package platform

import (
	"os"
	"os/exec"
	"runtime"
	"strings"
)

func PreferredLanguage(value string) string {
	value = strings.ToLower(strings.TrimSpace(value))
	if strings.HasPrefix(value, "fr") {
		return "fr"
	}
	return "en"
}
func SystemLanguage() string {
	if runtime.GOOS == "darwin" {
		if data, err := exec.Command("/usr/bin/defaults", "read", "-g", "AppleLanguages").Output(); err == nil {
			value := strings.TrimSpace(strings.Trim(string(data), "()\n \t"))
			value = strings.Trim(strings.Split(value, ",")[0], "\" \n\t")
			return PreferredLanguage(value)
		}
	}
	for _, key := range []string{"LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"} {
		if v := os.Getenv(key); v != "" {
			return PreferredLanguage(v)
		}
	}
	return "en"
}
