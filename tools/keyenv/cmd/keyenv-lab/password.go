package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"golang.org/x/term"
)

var errInteractionRequired = errors.New("INTERACTION_REQUIRED")
var errPasswordMismatch = errors.New("PASSWORD_MISMATCH")

// Only encrypted material is persisted; this fixture contains no real credentials.
type passwordFixture struct {
	Header     json.RawMessage `json:"header"`
	ObjectID   string          `json:"object_id"`
	Ciphertext []byte          `json:"ciphertext"`
}

const passwordPlaintext = "keyenv-lab manual password fixture; no real credentials"

func readPassword(prompt string) ([]byte, error) {
	if !term.IsTerminal(int(os.Stdin.Fd())) {
		return nil, errInteractionRequired
	}
	fmt.Fprint(os.Stderr, prompt)
	password, err := term.ReadPassword(int(os.Stdin.Fd()))
	fmt.Fprintln(os.Stderr)
	if err != nil {
		clear(password)
		return nil, errInteractionRequired
	}
	if len(password) == 0 || len(password) > 4096 {
		clear(password)
		return nil, vault.ErrInvalid
	}
	return password, nil
}

func makePasswordFixture(password []byte) ([]byte, error) {
	h, key, err := vault.Create(password, vault.DefaultKDF())
	if err != nil {
		return nil, err
	}
	defer clear(key)
	header, err := json.Marshal(h)
	if err != nil {
		return nil, err
	}
	id := vault.NewID()
	ciphertext, err := vault.Seal(key, h.VaultID, id, []byte(passwordPlaintext))
	if err != nil {
		return nil, err
	}
	return json.Marshal(passwordFixture{header, id, ciphertext})
}

func checkPasswordFixture(data, password []byte) error {
	if len(data) > vault.MaxObject {
		return vault.ErrInvalid
	}
	var f passwordFixture
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if d.Decode(&f) != nil || d.Decode(new(any)) != io.EOF {
		return vault.ErrInvalid
	}
	h, key, err := vault.Unlock(f.Header, password)
	if err != nil {
		return err
	}
	defer clear(key)
	plain, err := vault.Open(key, h.VaultID, f.ObjectID, f.Ciphertext)
	if err != nil {
		return err
	}
	defer clear(plain)
	if !bytes.Equal(plain, []byte(passwordPlaintext)) {
		return vault.ErrIntegrity
	}
	return nil
}

func passwordProbe(args []string) error {
	if len(args) == 0 || (args[0] != "create" && args[0] != "check") {
		return vault.ErrInvalid
	}
	flags := flag.NewFlagSet("password-probe", flag.ContinueOnError)
	flags.SetOutput(io.Discard)
	path := flags.String("file", "", "encrypted synthetic fixture path")
	if flags.Parse(args[1:]) != nil || flags.NArg() != 0 || *path == "" {
		return vault.ErrInvalid
	}
	password, err := readPassword("Test master password (hidden): ")
	if err != nil {
		return err
	}
	defer clear(password)
	if args[0] == "create" {
		confirm, err := readPassword("Confirm test master password (hidden): ")
		if err != nil {
			return err
		}
		same := bytes.Equal(password, confirm)
		clear(confirm)
		if !same {
			return errPasswordMismatch
		}
		data, err := makePasswordFixture(password)
		if err != nil {
			return err
		}
		if err := checkPasswordFixture(data, password); err != nil {
			return err
		}
		// Exclusive creation prevents overwriting an existing file or following a symlink.
		f, err := os.OpenFile(*path, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0600)
		if err != nil {
			return err
		}
		_, writeErr := f.Write(data)
		if writeErr == nil {
			writeErr = f.Sync()
		}
		closeErr := f.Close()
		if writeErr != nil {
			return writeErr
		}
		if closeErr != nil {
			return closeErr
		}
	} else {
		f, err := os.Open(*path)
		if err != nil {
			return err
		}
		defer f.Close()
		info, err := f.Stat()
		if err != nil {
			return err
		}
		if !info.Mode().IsRegular() || info.Size() > vault.MaxObject {
			return vault.ErrInvalid
		}
		data, err := io.ReadAll(io.LimitReader(f, vault.MaxObject+1))
		if err != nil {
			return err
		}
		if err := checkPasswordFixture(data, password); err != nil {
			return err
		}
	}
	return output(map[string]any{"result": "verified", "synthetic_data_only": true, "unlock_material_persisted": false})
}
