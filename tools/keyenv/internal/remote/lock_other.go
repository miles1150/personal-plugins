//go:build !(darwin || linux)

package remote

import "os"

// Other platforms run without cross-process refresh serialization.
func lockHome(home string) (func(), error) {
	return func() {}, os.MkdirAll(home, 0o700)
}
