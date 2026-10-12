package app

import (
	"encoding/json"
	"sort"
	"strings"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

// Export is not a shell script. Dollar and backtick characters remain literal.
func Export(entry vault.Entry, format, path string) error {
	if entry.Validate() != nil {
		return vault.ErrInvalid
	}
	if entry.Disabled {
		return vault.ErrDisabled
	}
	values := map[string]string{}
	keys := []string{}
	for k, f := range entry.Fields {
		values[k] = f.Value
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var data []byte
	switch format {
	case "json":
		var e error
		data, e = json.Marshal(values)
		if e != nil {
			return vault.ErrInvalid
		}
	case "dotenv":
		var b strings.Builder
		for _, k := range keys {
			b.WriteString(k)
			b.WriteByte('=')
			v := values[k]
			// dotenv parsers differ in expansion/escape rules. Refuse values which
			// cannot be represented literally across the supported simple dialect;
			// use JSON or direct process injection for arbitrary strings.
			if strings.ContainsAny(v, "\r\n\x00'\\$") {
				return vault.ErrInvalid
			}
			for _, r := range v {
				if r < 32 || r == 127 {
					return vault.ErrInvalid
				}
			}
			b.WriteByte('\'')
			b.WriteString(v)
			b.WriteByte('\'')
			b.WriteByte('\n')
		}
		data = []byte(b.String())
	default:
		return vault.ErrInvalid
	}
	defer clear(data)
	return exclusive(path, data)
}

func Environment(inherited []string, entry vault.Entry) ([]string, []string, error) {
	if entry.Validate() != nil {
		return nil, nil, vault.ErrInvalid
	}
	if entry.Disabled {
		return nil, nil, vault.ErrDisabled
	}
	var env, overridden []string
	seen := map[string]bool{}
	for _, v := range inherited {
		k, _, ok := strings.Cut(v, "=")
		if !ok {
			continue
		}
		if _, exists := entry.Fields[k]; exists {
			if !seen[k] {
				overridden = append(overridden, k)
				seen[k] = true
			}
			continue
		}
		env = append(env, v)
	}
	keys := []string{}
	for k := range entry.Fields {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		env = append(env, k+"="+entry.Fields[k].Value)
	}
	sort.Strings(overridden)
	return env, overridden, nil
}
