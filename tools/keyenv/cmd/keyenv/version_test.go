package main

import (
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestVersionAcceptsGlobalHomeWithoutOpeningVault(t *testing.T) {
	home := filepath.Join(t.TempDir(), "absent vault with spaces")
	for _, args := range [][]string{{"version"}, {"--home", home, "version"}, {"--home=" + home, "version"}} {
		t.Run(args[0], func(t *testing.T) {
			original := os.Stdout
			reader, writer, err := os.Pipe()
			if err != nil {
				t.Fatal(err)
			}
			defer reader.Close()
			defer writer.Close()
			os.Stdout = writer
			defer func() { os.Stdout = original }()
			err = run(args)
			writer.Close()
			os.Stdout = original
			if err != nil {
				t.Fatal(err)
			}
			data, err := io.ReadAll(reader)
			if err != nil {
				t.Fatal(err)
			}
			var got struct {
				Tool    string `json:"tool"`
				Version string `json:"version"`
			}
			if err := json.Unmarshal(data, &got); err != nil {
				t.Fatal(err)
			}
			if got.Tool != "keyenv" || got.Version != version {
				t.Fatalf("unexpected version response: %s", data)
			}
			if _, err := os.Stat(home); !os.IsNotExist(err) {
				t.Fatalf("version touched vault: %v", err)
			}
		})
	}
}

func TestVersionRejectsInvalidGlobalHomeAndCommandArguments(t *testing.T) {
	for _, args := range [][]string{{"--home", "relative", "version"}, {"version", "extra"}, {"--home", "/synthetic/absent", "version", "--offline"}} {
		if err := run(args); !errors.Is(err, vault.ErrInvalid) {
			t.Fatalf("%v: got %v", args, err)
		}
	}
}
