package app

import (
	"crypto/sha256"
	"encoding/hex"
	"sort"
	"strings"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

// Assignment values are literal, including quotes, dollar signs and '='.
// The caller obtains them only through the user's hidden terminal prompt.
func ParseAssignment(line []byte) (string, vault.Field, error) {
	key, value, ok := strings.Cut(string(line), "=")
	key = strings.TrimSpace(key)
	probe := vault.Entry{ID: "00000000000000000000000000000000", Name: "env/probe", Fields: map[string]vault.Field{key: {Value: value, Secret: true}}}
	if !ok || len(key) > 128 || probe.Validate() != nil || protectedGlobalKey(key) {
		return "", vault.Field{}, vault.ErrInvalid
	}
	return key, vault.Field{Value: value, Secret: true}, nil
}
func protectedGlobalKey(key string) bool {
	switch key {
	case "HOME", "PATH", "FPATH", "SHELL", "ZDOTDIR", "CODEX_HOME", "USER", "LOGNAME", "TMPDIR", "ENV", "BASH_ENV", "LD_PRELOAD", "LD_LIBRARY_PATH", "IFS", "CDPATH", "GLOBIGNORE", "SHELLOPTS", "BASHOPTS", "ZSH_VERSION", "ZSH_NAME", "path", "fpath", "status", "pipestatus", "argv", "commands", "functions", "aliases", "options", "history", "signals", "module_path", "modules":
		return true
	}
	return strings.HasPrefix(key, "DYLD_") || strings.HasPrefix(key, "_keyenv_")
}

// GlobalFields flattens active entries without silently picking a duplicate or
// conflicted version. Pending revisions are excluded by Engine.Cached.
func GlobalFields(revs map[string]vault.Revision) (map[string]vault.Field, error) {
	heads, err := vault.Heads(revs)
	if err != nil {
		return nil, err
	}
	fields := map[string]vault.Field{}
	for _, ids := range heads {
		if len(ids) != 1 {
			return nil, vault.ErrConflict
		}
		entry := revs[ids[0]].Entry
		if entry.Disabled {
			continue
		}
		for key, field := range entry.Fields {
			if protectedGlobalKey(key) {
				return nil, vault.ErrInvalid
			}
			if _, exists := fields[key]; exists {
				return nil, vault.ErrConflict
			}
			fields[key] = field
		}
	}
	return fields, nil
}

type AssignmentPlan struct {
	Entry   vault.Entry
	Parents []string
	Keys    []string
}

// Existing service groups remain intact. New flat keys each have their own
// entry, so edits to different keys on different devices can coexist.
func PlanAssignments(revs map[string]vault.Revision, values map[string]vault.Field) ([]AssignmentPlan, error) {
	heads, err := vault.Heads(revs)
	if err != nil {
		return nil, err
	}
	names := map[string]string{}
	for name, ids := range heads {
		if len(ids) != 1 {
			return nil, vault.ErrConflict
		}
		entry := revs[ids[0]].Entry
		for key := range entry.Fields {
			if _, exists := values[key]; !exists {
				continue
			}
			if _, exists := names[key]; exists {
				return nil, vault.ErrConflict
			}
			if entry.Disabled {
				return nil, vault.ErrDisabled
			}
			names[key] = name
		}
	}
	plans := map[string]*AssignmentPlan{}
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	for _, key := range keys {
		field := values[key]
		probe := vault.Entry{ID: "00000000000000000000000000000000", Name: "env/probe", Fields: map[string]vault.Field{key: field}}
		if len(key) > 128 || probe.Validate() != nil || protectedGlobalKey(key) {
			return nil, vault.ErrInvalid
		}
		name, exists := names[key]
		if !exists {
			hash := sha256.Sum256([]byte(key))
			name = "env/" + hex.EncodeToString(hash[:])
			if len(heads[name]) != 0 {
				return nil, vault.ErrConflict
			}
		}
		plan := plans[name]
		if plan == nil {
			entry := vault.Entry{ID: vault.NewID(), Name: name, Fields: map[string]vault.Field{}}
			if exists {
				entry = revs[heads[name][0]].Entry
				copied := map[string]vault.Field{}
				for k, v := range entry.Fields {
					copied[k] = v
				}
				entry.Fields = copied
			}
			plan = &AssignmentPlan{Entry: entry, Parents: append([]string(nil), heads[name]...)}
			plans[name] = plan
		}
		plan.Entry.Fields[key] = field
		plan.Keys = append(plan.Keys, key)
	}
	result := make([]AssignmentPlan, 0, len(plans))
	ordered := make([]string, 0, len(plans))
	for name := range plans {
		ordered = append(ordered, name)
	}
	sort.Strings(ordered)
	for _, name := range ordered {
		if plans[name].Entry.Validate() != nil {
			return nil, vault.ErrInvalid
		}
		result = append(result, *plans[name])
	}
	return result, nil
}
func ShellQuote(value string) string { return "'" + strings.ReplaceAll(value, "'", "'\"'\"'") + "'" }
func ZshEnvironment(fields map[string]vault.Field) ([]byte, error) {
	probe := vault.Entry{ID: "00000000000000000000000000000000", Name: "env/probe", Fields: fields}
	if len(fields) == 0 {
		return nil, nil
	}
	// The flattened pool may contain more than 64 fields; validate one at a time.
	keys := make([]string, 0, len(fields))
	for key, field := range fields {
		probe.Fields = map[string]vault.Field{key: field}
		if probe.Validate() != nil || protectedGlobalKey(key) {
			return nil, vault.ErrInvalid
		}
		keys = append(keys, key)
	}
	sort.Strings(keys)
	var result strings.Builder
	for _, key := range keys {
		result.WriteString("builtin export ")
		result.WriteString(key)
		result.WriteByte('=')
		result.WriteString(ShellQuote(fields[key].Value))
		result.WriteByte('\n')
	}
	result.WriteString("typeset -ga _keyenv_loaded_keys=(")
	for _, key := range keys {
		result.WriteString(ShellQuote(key))
		result.WriteByte(' ')
	}
	result.WriteString(")\n")
	return []byte(result.String()), nil
}
