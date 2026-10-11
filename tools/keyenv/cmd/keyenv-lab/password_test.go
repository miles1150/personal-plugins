package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestPasswordFixture(t *testing.T) {
	password := []byte("synthetic-test-password-DO-NOT-USE")
	data, err := makePasswordFixture(password)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(data, password) || bytes.Contains(data, []byte(passwordPlaintext)) {
		t.Fatal("plaintext persisted")
	}
	if err := checkPasswordFixture(data, password); err != nil {
		t.Fatal(err)
	}
	if err := checkPasswordFixture(data, []byte("wrong")); !errors.Is(err, vault.ErrAuthentication) {
		t.Fatalf("wrong password: %v", err)
	}
	var f passwordFixture
	if err := json.Unmarshal(data, &f); err != nil {
		t.Fatal(err)
	}
	f.ObjectID = vault.NewID()
	altered, _ := json.Marshal(f)
	if err := checkPasswordFixture(altered, password); !errors.Is(err, vault.ErrAuthentication) {
		t.Fatalf("rebound object: %v", err)
	}
	if err := checkPasswordFixture(append(data, []byte(" {}")...), password); !errors.Is(err, vault.ErrInvalid) {
		t.Fatal("accepted trailing data")
	}
}
