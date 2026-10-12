package app

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

const profileStart = "# >>> keyenv managed environment >>>"
const profileEnd = "# <<< keyenv managed environment <<<"

// ProfileHook contains only paths and a loader. Decrypted exports exist only in
// the shell's local memory; tracing is disabled for loading and restored on return.
func ProfileHook(executable, home string) ([]byte, error) {
	if !filepath.IsAbs(executable) || !filepath.IsAbs(home) || stringsInvalidPath(executable) || stringsInvalidPath(home) {
		return nil, vault.ErrInvalid
	}
	return []byte(fmt.Sprintf(`%s
function keyenv() {
  %s --home %s "$@" || return $?
  case "${1-}" in
    set|update|disable|resolve|sync) keyenv_load || return $? ;;
  esac
  return 0
}
function keyenv_load() {
  builtin setopt localoptions noxtrace noaliases
  local _keyenv_exports _keyenv_key
  _keyenv_exports="$(%s --home %s env --offline --format zsh)" || return $?
  for _keyenv_key in "${_keyenv_loaded_keys[@]-}"; do
    if [[ -n $_keyenv_key ]]; then
      builtin unset "$_keyenv_key"
    fi
  done
  typeset -ga _keyenv_loaded_keys=()
  builtin eval "$_keyenv_exports"
}
if [[ -x %s ]]; then
  keyenv_load
fi
%s
`, profileStart, ShellQuote(executable), ShellQuote(home), ShellQuote(executable), ShellQuote(home), ShellQuote(executable), profileEnd)), nil
}
func stringsInvalidPath(path string) bool { return strings.ContainsAny(path, "\r\n\x00") }

// InstallProfile preserves other profile contents and private backups. Re-running
// it replaces only one marked block. The caller explicitly chooses this file.
func (s *Session) InstallProfile(path, executable string) (string, error) {
	if !filepath.IsAbs(path) || privateDir(s.Home) != nil {
		return "", vault.ErrInvalid
	}
	hook, err := ProfileHook(executable, s.Home)
	if err != nil {
		return "", err
	}
	var original []byte
	mode := os.FileMode(0600)
	exists := false
	if info, e := os.Lstat(path); e == nil {
		if !info.Mode().IsRegular() || info.Size() > 1024*1024 {
			return "", ErrStorage
		}
		exists = true
		mode = info.Mode().Perm()
		original, err = os.ReadFile(path)
		if err != nil {
			return "", ErrStorage
		}
	} else if !os.IsNotExist(e) {
		return "", ErrStorage
	}
	defer clear(original)
	start, end := bytes.Index(original, []byte(profileStart)), bytes.Index(original, []byte(profileEnd))
	data := append([]byte(nil), original...)
	defer func() { clear(data) }()
	if start >= 0 || end >= 0 {
		if start < 0 || end < start || bytes.Count(original, []byte(profileStart)) != 1 || bytes.Count(original, []byte(profileEnd)) != 1 {
			return "", vault.ErrInvalid
		}
		tail := end + len(profileEnd)
		if tail < len(original) && original[tail] == '\n' {
			tail++
		}
		data = append(append(append([]byte(nil), original[:start]...), hook...), original[tail:]...)
	} else {
		if len(data) > 0 && data[len(data)-1] != '\n' {
			data = append(data, '\n')
		}
		data = append(data, '\n')
		data = append(data, hook...)
	}
	if bytes.Equal(original, data) {
		return "", nil
	}
	backup := ""
	if exists {
		backup = filepath.Join(s.Home, "shell-profile-backup-"+time.Now().UTC().Format("20060102T150405.000000000")+".txt")
		if err = exclusive(backup, original); err != nil {
			return "", err
		}
	}
	f, err := os.CreateTemp(filepath.Dir(path), ".keyenv-profile-*")
	if err != nil {
		return backup, ErrStorage
	}
	defer f.Close()
	defer os.Remove(f.Name())
	if f.Chmod(mode) != nil {
		return backup, ErrStorage
	}
	if _, err = f.Write(data); err != nil || f.Sync() != nil || f.Close() != nil {
		return backup, ErrStorage
	}
	if exists {
		info, e := os.Lstat(path)
		if e != nil || !info.Mode().IsRegular() {
			return backup, ErrStorage
		}
		current, e := os.ReadFile(path)
		same := e == nil && bytes.Equal(current, original)
		clear(current)
		if !same {
			return backup, vault.ErrConflict
		}
		if os.Rename(f.Name(), path) != nil {
			return backup, ErrStorage
		}
	} else {
		if err = os.Link(f.Name(), path); err != nil {
			if os.IsExist(err) {
				return backup, ErrExists
			}
			return backup, ErrStorage
		}
	}
	return backup, syncDir(filepath.Dir(path))
}
