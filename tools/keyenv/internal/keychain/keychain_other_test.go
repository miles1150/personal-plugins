//go:build !darwin || !cgo

package keychain

import (
	"errors"
	"testing"
)

func TestUnsupportedPlatformFailsClosed(t *testing.T) {
	if !errors.Is(Put("test", []byte("secret")), ErrUnsupported) {
		t.Fatal("unexpected fallback")
	}
	if _, err := Get("test"); !errors.Is(err, ErrUnsupported) {
		t.Fatal("unexpected fallback")
	}
	if !errors.Is(Delete("test"), ErrUnsupported) {
		t.Fatal("unexpected fallback")
	}
}
