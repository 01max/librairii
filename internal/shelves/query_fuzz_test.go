package shelves

import (
	"errors"
	"reflect"
	"testing"

	"github.com/01max/librairii/internal/shelfquery"
)

func FuzzSavedLibraryQueryCodecPreservesMembership(f *testing.F) {
	for _, seed := range []struct {
		version int
		payload string
	}{
		{version: 2, payload: `{}`},
		{
			version: 2,
			payload: `{"name":"forest","languages":["en-GB"],` +
				`"booleanFilters":[{"definitionId":2,"state":"true"}],` +
				`"choiceFilters":[{"definitionId":5,"valueIds":[3,7]}]}`,
		},
		{
			version: 1,
			payload: `{"name":"legacy","page":7,"pageSize":12,` +
				`"sort":"imported_desc","choiceFilters":[` +
				`{"definitionId":4,"valueIds":[9]}]}`,
		},
		{version: 2, payload: `{"choiceFilters":[],"choiceFilters":[{"definitionId":1,"valueIds":[2]}]}`},
		{version: 2, payload: `{"selection":[1]}`},
		{version: 99, payload: `{}`},
	} {
		f.Add(seed.version, seed.payload)
	}

	f.Fuzz(func(t *testing.T, version int, payload string) {
		decoded, err := DecodeSavedLibraryQuery(version, payload)
		if err != nil {
			if version != 1 &&
				version != CurrentSavedLibraryQueryVersion &&
				!errors.Is(err, ErrUnsupportedSavedQueryVersion) {
				t.Fatalf(
					"unsupported version %d returned %v",
					version,
					err,
				)
			}
			return
		}
		if version != 1 && version != CurrentSavedLibraryQueryVersion {
			t.Fatalf("unsupported version %d decoded successfully", version)
		}
		encoded, err := EncodeSavedLibraryQuery(decoded.StoryLibraryQuery())
		if err != nil {
			t.Fatalf("EncodeSavedLibraryQuery(decoded) error = %v", err)
		}
		if encoded.Version != shelfquery.CurrentVersion ||
			len(encoded.Payload) > shelfquery.MaxPayloadBytes {
			t.Fatalf("canonical saved query = %#v", encoded)
		}
		roundTrip, err := DecodeSavedLibraryQuery(
			encoded.Version,
			encoded.Payload,
		)
		if err != nil {
			t.Fatalf("DecodeSavedLibraryQuery(canonical) error = %v", err)
		}
		if !reflect.DeepEqual(roundTrip, decoded) {
			t.Fatalf(
				"saved codec broadened membership: decoded=%#v roundTrip=%#v",
				decoded,
				roundTrip,
			)
		}
		reencoded, err := EncodeSavedLibraryQuery(roundTrip.StoryLibraryQuery())
		if err != nil || reencoded != encoded {
			t.Fatalf(
				"canonical saved query changed: first=%#v second=%#v error=%v",
				encoded,
				reencoded,
				err,
			)
		}
	})
}
