//go:build !darwin && !linux

package app

import "github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"

func lock(home string) (func(), error) { return nil, vault.ErrUnsupported }
