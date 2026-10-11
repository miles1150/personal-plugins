//go:build darwin && cgo

package keychain

/*
#cgo LDFLAGS: -framework Security -framework CoreFoundation
#cgo CFLAGS: -Wno-deprecated-declarations
#include <Security/Security.h>
#include <stdlib.h>
#include <string.h>

static OSStatus kv_put(const char *path, const char *service, const char *account, const void *value, UInt32 length) {
    SecKeychainRef kc = NULL;
    OSStatus status = SecKeychainOpen(path, &kc);
    if (status != errSecSuccess) return status;
    SecKeychainItemRef item = NULL;
    status = SecKeychainFindGenericPassword(kc, (UInt32)strlen(service), service, (UInt32)strlen(account), account, NULL, NULL, &item);
    if (status == errSecSuccess) {
        status = SecKeychainItemModifyAttributesAndData(item, NULL, length, value);
        CFRelease(item);
    } else if (status == errSecItemNotFound) {
        status = SecKeychainAddGenericPassword(kc, (UInt32)strlen(service), service, (UInt32)strlen(account), account, length, value, NULL);
    }
    CFRelease(kc);
    return status;
}
static OSStatus kv_get(const char *path, const char *service, const char *account, UInt32 *length, void **value) {
    SecKeychainRef kc = NULL;
    OSStatus status = SecKeychainOpen(path, &kc);
    if (status != errSecSuccess) return status;
    status = SecKeychainFindGenericPassword(kc, (UInt32)strlen(service), service, (UInt32)strlen(account), account, length, value, NULL);
    CFRelease(kc);
    return status;
}
static OSStatus kv_delete(const char *path, const char *service, const char *account) {
    SecKeychainRef kc = NULL;
    OSStatus status = SecKeychainOpen(path, &kc);
    if (status != errSecSuccess) return status;
    SecKeychainItemRef item = NULL;
    status = SecKeychainFindGenericPassword(kc, (UInt32)strlen(service), service, (UInt32)strlen(account), account, NULL, NULL, &item);
    if (status == errSecSuccess) {
        status = SecKeychainItemDelete(item);
        CFRelease(item);
    }
    CFRelease(kc);
    return status;
}
*/
import "C"

import (
	"os"
	"path/filepath"
	"strings"
	"unsafe"
)

func args(account string) (*C.char, *C.char, *C.char, func(), error) {
	if len(account) == 0 || len(account) > 160 || strings.ContainsRune(account, 0) {
		return nil, nil, nil, nil, ErrAccess
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return nil, nil, nil, nil, ErrAccess
	}
	p, s, a := C.CString(filepath.Join(home, "Library", "Keychains", "login.keychain-db")), C.CString(service), C.CString(account)
	return p, s, a, func() { C.free(unsafe.Pointer(p)); C.free(unsafe.Pointer(s)); C.free(unsafe.Pointer(a)) }, nil
}

func status(code C.OSStatus) error {
	if code == C.errSecSuccess {
		return nil
	}
	if code == C.errSecItemNotFound {
		return ErrMissing
	}
	return ErrAccess
}

func Put(account string, value []byte) error {
	if len(value) == 0 || len(value) > 65536 {
		return ErrAccess
	}
	p, s, a, done, err := args(account)
	if err != nil {
		return err
	}
	defer done()
	return status(C.kv_put(p, s, a, unsafe.Pointer(&value[0]), C.UInt32(len(value))))
}

func Get(account string) ([]byte, error) {
	p, s, a, done, err := args(account)
	if err != nil {
		return nil, err
	}
	defer done()
	var size C.UInt32
	var ptr unsafe.Pointer
	if err := status(C.kv_get(p, s, a, &size, &ptr)); err != nil {
		return nil, err
	}
	defer C.SecKeychainItemFreeContent(nil, ptr)
	if size > 65536 {
		return nil, ErrAccess
	}
	return C.GoBytes(ptr, C.int(size)), nil
}

func Delete(account string) error {
	p, s, a, done, err := args(account)
	if err != nil {
		return err
	}
	defer done()
	return status(C.kv_delete(p, s, a))
}
