// Package vault implements an experimental, versioned encrypted object format.
// The format is a prototype, not a production compatibility commitment.
package vault

import (
	"bytes"
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"

	"golang.org/x/crypto/argon2"
)

const MaxObject = 1 << 20

var (
	ErrInvalid        = errors.New("INVALID_DATA")
	ErrAuthentication = errors.New("DECRYPTION_FAILED")
	ErrUnsupported    = errors.New("UNSUPPORTED_FORMAT")
)

type KDF struct {
	Memory  uint32 `json:"memory_kib"`
	Time    uint32 `json:"iterations"`
	Threads uint8  `json:"threads"`
}

func DefaultKDF() KDF { return KDF{Memory: 64 * 1024, Time: 3, Threads: 2} }

func (p KDF) valid() bool {
	return p.Memory >= 8*1024 && p.Memory <= 128*1024 && p.Time >= 1 && p.Time <= 6 && p.Threads >= 1 && p.Threads <= 4
}

type Header struct {
	Version    int    `json:"version"`
	VaultID    string `json:"vault_id"`
	KDF        KDF    `json:"kdf"`
	Salt       []byte `json:"salt"`
	Nonce      []byte `json:"nonce"`
	WrappedKey []byte `json:"wrapped_key"`
}

type envelope struct {
	Version    int    `json:"version"`
	Nonce      []byte `json:"nonce"`
	Ciphertext []byte `json:"ciphertext"`
}

func NewID() string {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return hex.EncodeToString(b)
}

func random(n int) ([]byte, error) {
	b := make([]byte, n)
	_, err := rand.Read(b)
	return b, err
}

func gcm(key []byte) (cipher.AEAD, error) {
	if len(key) != 32 {
		return nil, ErrInvalid
	}
	b, err := aes.NewCipher(key)
	if err != nil {
		return nil, ErrInvalid
	}
	return cipher.NewGCM(b)
}

// decode rejects oversized input, unknown fields, and trailing JSON values.
func decode(data []byte, dst any) error {
	if len(data) > MaxObject {
		return ErrInvalid
	}
	d := json.NewDecoder(bytes.NewReader(data))
	d.DisallowUnknownFields()
	if err := d.Decode(dst); err != nil {
		return ErrInvalid
	}
	if err := d.Decode(new(any)); err != io.EOF {
		return ErrInvalid
	}
	return nil
}

func headerAAD(h Header) []byte {
	b, _ := json.Marshal(struct {
		Version int
		VaultID string
		KDF     KDF
		Salt    []byte
	}{h.Version, h.VaultID, h.KDF, h.Salt})
	return append([]byte("keyenv-lab/header/"), b...)
}

func Create(password []byte, params KDF) (Header, []byte, error) {
	if len(password) == 0 || len(password) > 4096 || !params.valid() {
		return Header{}, nil, ErrInvalid
	}
	h := Header{Version: 1, VaultID: NewID(), KDF: params}
	var err error
	if h.Salt, err = random(16); err != nil {
		return Header{}, nil, err
	}
	key, err := random(32)
	if err != nil {
		return Header{}, nil, err
	}
	kek := argon2.IDKey(password, h.Salt, params.Time, params.Memory, params.Threads, 32)
	defer clear(kek)
	aead, _ := gcm(kek)
	if h.Nonce, err = random(aead.NonceSize()); err != nil {
		clear(key)
		return Header{}, nil, err
	}
	h.WrappedKey = aead.Seal(nil, h.Nonce, key, headerAAD(h))
	return h, key, nil
}

// ParseHeader validates format and resource bounds without deriving a password key.
func ParseHeader(data []byte) (Header, error) {
	var h Header
	if err := decode(data, &h); err != nil {
		return h, err
	}
	if h.Version != 1 {
		return h, ErrUnsupported
	}
	if len(h.VaultID) != 32 || !h.KDF.valid() || len(h.Salt) != 16 || len(h.Nonce) != 12 || len(h.WrappedKey) != 48 {
		return h, ErrInvalid
	}
	if _, err := hex.DecodeString(h.VaultID); err != nil {
		return h, ErrInvalid
	}
	return h, nil
}

func Unlock(data, password []byte) (Header, []byte, error) {
	h, err := ParseHeader(data)
	if err != nil {
		return h, nil, err
	}
	if len(password) == 0 || len(password) > 4096 {
		return h, nil, ErrInvalid
	}
	kek := argon2.IDKey(password, h.Salt, h.KDF.Time, h.KDF.Memory, h.KDF.Threads, 32)
	defer clear(kek)
	aead, _ := gcm(kek)
	key, err := aead.Open(nil, h.Nonce, h.WrappedKey, headerAAD(h))
	if err != nil {
		return h, nil, ErrAuthentication
	}
	return h, key, nil
}

func objectAAD(vaultID, objectID string) []byte {
	b, _ := json.Marshal([]string{"keyenv-lab/object/v1", vaultID, objectID})
	return b
}

func Seal(key []byte, vaultID, objectID string, plaintext []byte) ([]byte, error) {
	if !ValidID(vaultID) || !ValidID(objectID) || len(plaintext) > MaxObject/2 {
		return nil, ErrInvalid
	}
	aead, err := gcm(key)
	if err != nil {
		return nil, err
	}
	nonce, err := random(aead.NonceSize())
	if err != nil {
		return nil, err
	}
	return json.Marshal(envelope{Version: 1, Nonce: nonce, Ciphertext: aead.Seal(nil, nonce, plaintext, objectAAD(vaultID, objectID))})
}

func Open(key []byte, vaultID, objectID string, data []byte) ([]byte, error) {
	var e envelope
	if err := decode(data, &e); err != nil {
		return nil, err
	}
	if e.Version != 1 {
		return nil, ErrUnsupported
	}
	aead, err := gcm(key)
	if err != nil {
		return nil, err
	}
	if len(e.Nonce) != aead.NonceSize() {
		return nil, ErrInvalid
	}
	b, err := aead.Open(nil, e.Nonce, e.Ciphertext, objectAAD(vaultID, objectID))
	if err != nil {
		return nil, ErrAuthentication
	}
	return b, nil
}

func ValidID(s string) bool {
	if len(s) < 8 || len(s) > 160 {
		return false
	}
	for _, r := range s {
		if !(r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= '0' && r <= '9' || r == '-' || r == '_') {
			return false
		}
	}
	return true
}
