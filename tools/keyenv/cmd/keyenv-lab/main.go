// keyenv-lab runs synthetic verification probes. It is not the production keyenv CLI.
package main

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/drive"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/keychain"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func main() {
	if err := run(os.Args[1:]); err != nil {
		// Only known static error codes are exposed; arbitrary library errors are suppressed.
		code := "PROBE_FAILED"
		for _, known := range []error{vault.ErrInvalid, vault.ErrAuthentication, vault.ErrUnsupported, vault.ErrUnavailable, vault.ErrAuthRequired, vault.ErrIntegrity, vault.ErrConflict, keychain.ErrUnsupported, keychain.ErrAccess, keychain.ErrMissing} {
			if errors.Is(err, known) {
				code = known.Error()
				break
			}
		}
		_ = json.NewEncoder(os.Stderr).Encode(map[string]string{"error": code})
		os.Exit(1)
	}
}

func output(v any) error { return json.NewEncoder(os.Stdout).Encode(v) }

func run(args []string) error {
	if len(args) == 0 {
		return output(map[string]any{"tool": "keyenv-lab", "synthetic_data_only": true, "commands": []string{"simulate", "drive-probe --oauth-client PATH [--folder ID]", "keychain-probe create|check|delete [--id ID]"}})
	}
	switch args[0] {
	case "simulate":
		if len(args) != 1 {
			return vault.ErrInvalid
		}
		report, err := lab.Simulate()
		if err != nil {
			return err
		}
		return output(report)
	case "drive-probe":
		return driveProbe(args[1:])
	case "keychain-probe":
		return keychainProbe(args[1:])
	default:
		return vault.ErrInvalid
	}
}

func driveProbe(args []string) error {
	flags := flag.NewFlagSet("drive-probe", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	client := flags.String("oauth-client", "", "Desktop OAuth client JSON path")
	folder := flags.String("folder", "", "existing lab folder ID from first Mac")
	if err := flags.Parse(args); err != nil || flags.NArg() != 0 || *client == "" {
		return vault.ErrInvalid
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()
	httpClient, err := drive.AuthorizeLab(ctx, *client, os.Stderr)
	if err != nil {
		return err
	}
	s := &drive.Store{Client: httpClient, FolderID: *folder}
	if *folder == "" {
		s.FolderID, err = s.CreateLabFolder(ctx)
	} else {
		err = s.CheckLabFolder(ctx)
	}
	if err != nil {
		return err
	}
	fmt.Fprintln(os.Stderr, "Lab folder ID:", s.FolderID)
	ids, err := s.List(ctx)
	if err != nil {
		return err
	}
	for _, id := range ids {
		data, err := s.Get(ctx, id)
		if err != nil {
			return err
		}
		plain, err := vault.Open(lab.FixtureKey(), lab.FixtureVault, id, data)
		if err != nil {
			return err
		}
		if string(plain) != lab.FixturePlaintext {
			return vault.ErrIntegrity
		}
	}
	id, err := s.Reserve(ctx)
	if err != nil {
		return err
	}
	sealed, err := vault.Seal(lab.FixtureKey(), lab.FixtureVault, id, []byte(lab.FixturePlaintext))
	if err != nil {
		return err
	}
	if err = s.Put(ctx, id, sealed); err != nil {
		return err
	}
	if err = s.Put(ctx, id, sealed); err != nil {
		return err
	} // Actual 409/idempotent-retry path.
	actual, err := s.Get(ctx, id)
	if err != nil {
		return err
	}
	if !bytes.Equal(actual, sealed) {
		return vault.ErrIntegrity
	}
	return output(map[string]any{"folder_id": s.FolderID, "created_file_id": id, "existing_fixtures_verified": len(ids), "idempotent_retry": true, "synthetic_data_only": true, "login_persisted": false})
}

func keychainProbe(args []string) error {
	if len(args) == 0 {
		return vault.ErrInvalid
	}
	flags := flag.NewFlagSet("keychain-probe", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	id := flags.String("id", "", "probe ID returned by create")
	if err := flags.Parse(args[1:]); err != nil || flags.NArg() != 0 {
		return vault.ErrInvalid
	}
	if args[0] == "create" {
		if *id != "" {
			return vault.ErrInvalid
		}
		*id = vault.NewID()
		b := make([]byte, 32)
		if _, err := rand.Read(b); err != nil {
			return err
		}
		defer clear(b)
		if err := keychain.Put("probe/"+*id, b); err != nil {
			return err
		}
		stored, err := keychain.Get("probe/" + *id)
		if err != nil {
			return err
		}
		defer clear(stored)
		if !bytes.Equal(stored, b) {
			return vault.ErrIntegrity
		}
		sum := sha256.Sum256(stored)
		return output(map[string]string{"probe_id": *id, "fingerprint": hex.EncodeToString(sum[:]), "result": "created; use check after logout or rebuild; delete when done"})
	}
	if !vault.ValidID(*id) {
		return vault.ErrInvalid
	}
	switch args[0] {
	case "check":
		b, err := keychain.Get("probe/" + *id)
		if err != nil {
			return err
		}
		defer clear(b)
		sum := sha256.Sum256(b)
		return output(map[string]string{"probe_id": *id, "fingerprint": hex.EncodeToString(sum[:])})
	case "delete":
		if err := keychain.Delete("probe/" + *id); err != nil {
			return err
		}
		return output(map[string]string{"result": "deleted"})
	default:
		return vault.ErrInvalid
	}
}
