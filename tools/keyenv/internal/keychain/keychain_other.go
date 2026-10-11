//go:build !darwin || !cgo

package keychain

func Put(account string, value []byte) error { return ErrUnsupported }
func Get(account string) ([]byte, error)     { return nil, ErrUnsupported }
func Delete(account string) error            { return ErrUnsupported }
