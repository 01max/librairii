package library

import (
	"reflect"
	"testing"
)

func FuzzStoryLibraryQueryCodecCanonicalRoundTrip(f *testing.F) {
	for _, hash := range []string{
		"",
		"#/library?v=1",
		"#/library?name=forest&v=1",
		"#/library?bool=2%3Atrue&choice=5%3A3%2C7&v=1",
		"#/library?v=2",
		"#/library?v=1&unknown=value",
		"#/library?v=1&choice=1%3A",
		"#/library?v=1&bool=1%3Atrue&bool=1%3Afalse",
	} {
		f.Add(hash)
	}

	f.Fuzz(func(t *testing.T, hash string) {
		decoded, err := DecodeStoryLibraryQuery(hash)
		if err != nil {
			return
		}
		encoded, err := EncodeStoryLibraryQuery(decoded)
		if err != nil {
			t.Fatalf("EncodeStoryLibraryQuery(decoded %q) error = %v", hash, err)
		}
		roundTrip, err := DecodeStoryLibraryQuery(encoded)
		if err != nil {
			t.Fatalf("DecodeStoryLibraryQuery(canonical %q) error = %v", encoded, err)
		}
		if !reflect.DeepEqual(roundTrip, decoded) {
			t.Fatalf(
				"codec broadened query: decoded=%#v roundTrip=%#v",
				decoded,
				roundTrip,
			)
		}
		reencoded, err := EncodeStoryLibraryQuery(roundTrip)
		if err != nil || reencoded != encoded {
			t.Fatalf(
				"canonical encoding changed: first=%q second=%q error=%v",
				encoded,
				reencoded,
				err,
			)
		}
	})
}
