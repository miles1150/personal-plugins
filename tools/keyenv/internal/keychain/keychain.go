// Package keychain stores bytes in the local macOS login keychain, not iCloud Keychain.
package keychain

import "errors"

var (
	ErrUnsupported = errors.New("MACOS_KEYCHAIN_REQUIRED")
	ErrAccess      = errors.New("KEYCHAIN_ACCESS_FAILED")
	ErrMissing     = errors.New("KEYCHAIN_ITEM_NOT_FOUND")
)

const service = "dev.personal-plugins.keyenv-lab"
