package vault

import (
	"bytes"
	"encoding/json"
	"errors"
	"strings"
	"testing"
)

func TestPasswordAndObjectBinding(t *testing.T) {
	password := []byte("synthetic-master-password")
	h, key, err := Create(password, KDF{Memory: 8192, Time: 1, Threads: 1})
	if err != nil {
		t.Fatal(err)
	}
	header, _ := json.Marshal(h)
	_, recovered, err := Unlock(header, password)
	if err != nil || !bytes.Equal(key, recovered) {
		t.Fatal("password roundtrip failed", err)
	}
	if _, _, err = Unlock(header, []byte("wrong")); !errors.Is(err, ErrAuthentication) {
		t.Fatal("wrong password accepted")
	}
	objectID := NewID()
	secret := []byte(`{"token":"synthetic-private-value","endpoint":"https://example.invalid"}`)
	blob, err := Seal(key, h.VaultID, objectID, secret)
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(blob, []byte("synthetic-private-value")) || bytes.Contains(header, key) {
		t.Fatal("plaintext leaked")
	}
	plain, err := Open(recovered, h.VaultID, objectID, blob)
	if err != nil || !bytes.Equal(plain, secret) {
		t.Fatal(err)
	}
	for _, tc := range []struct{ name, vault, id string }{{"other-vault", NewID(), objectID}, {"other-object", h.VaultID, NewID()}} {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := Open(key, tc.vault, tc.id, blob); !errors.Is(err, ErrAuthentication) {
				t.Fatal("substitution accepted", err)
			}
		})
	}
	var e envelope
	_ = json.Unmarshal(blob, &e)
	e.Ciphertext[0] ^= 1
	tampered, _ := json.Marshal(e)
	if _, err := Open(key, h.VaultID, objectID, tampered); !errors.Is(err, ErrAuthentication) {
		t.Fatal("tampered data accepted")
	}
	second, _ := Seal(key, h.VaultID, objectID, secret)
	if bytes.Equal(blob, second) {
		t.Fatal("nonce reused")
	}
}

func TestUntrustedBoundsAndFormat(t *testing.T) {
	h, _, _ := Create([]byte("fixture"), KDF{Memory: 8192, Time: 1, Threads: 1})
	h.KDF.Memory = ^uint32(0)
	b, _ := json.Marshal(h)
	if _, _, err := Unlock(b, []byte("fixture")); !errors.Is(err, ErrInvalid) {
		t.Fatal("unbounded KDF accepted")
	}
	h.KDF = DefaultKDF()
	h.Version = 2
	b, _ = json.Marshal(h)
	if _, _, err := Unlock(b, []byte("fixture")); !errors.Is(err, ErrUnsupported) {
		t.Fatal("future version accepted")
	}
	key := make([]byte, 32)
	for _, bad := range [][]byte{[]byte(strings.Repeat("x", MaxObject+1)), []byte(`{"version":1,"nonce":"","ciphertext":""}`), []byte(`{} {}`), []byte(`{"version":1,"unknown":true}`)} {
		if _, err := Open(key, NewID(), NewID(), bad); err == nil {
			t.Fatal("bad envelope accepted")
		}
	}
}

func FuzzOpen(f *testing.F) {
	f.Add([]byte(`{"version":1,"nonce":"","ciphertext":""}`))
	f.Add([]byte("not json"))
	f.Fuzz(func(t *testing.T, b []byte) { _, _ = Open(make([]byte, 32), "fixture-vault", "fixture-object", b) })
}
