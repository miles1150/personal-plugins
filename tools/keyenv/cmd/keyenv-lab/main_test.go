package main

import (
	"errors"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestReadOnlyProbeRequiresExplicitFolderBeforeAuthorization(t *testing.T) {
	for _, args := range [][]string{
		{"--oauth-client", "/missing-client.json", "--read-only"},
		{"--oauth-client", "/missing-client.json", "--folder", "test-folder", "--page-size", "0"},
		{"--oauth-client", "/missing-client.json", "--folder", "test-folder", "--page-size", "101"},
	} {
		if err := driveProbe(args); !errors.Is(err, vault.ErrInvalid) {
			t.Fatal("unsafe probe options reached authorization", err)
		}
	}
}
