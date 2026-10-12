package vault

import (
	"errors"
	"regexp"
	"sort"
	"strings"
	"time"
)

var (
	ErrConflict = errors.New("CONFLICT")
	ErrDisabled = errors.New("DISABLED")
	ErrNotFound = errors.New("NOT_FOUND")
	namePattern = regexp.MustCompile(`^[a-z][a-z0-9/_-]{0,79}$`)
	envPattern  = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)
)

type Field struct {
	Value  string `json:"value"`
	Secret bool   `json:"secret"`
}

type Entry struct {
	ID       string           `json:"id"`
	Name     string           `json:"name"`
	Purpose  string           `json:"purpose"`
	Disabled bool             `json:"disabled"`
	Expires  string           `json:"expires,omitempty"`
	Fields   map[string]Field `json:"fields"`
}

type Revision struct {
	ID      string   `json:"id"`
	Device  string   `json:"device"`
	Parents []string `json:"parents"`
	Entry   Entry    `json:"entry"`
	// Resolution explicitly merges same-name branches, including independently created IDs.
	Resolution bool `json:"resolution,omitempty"`
}

func (e Entry) Validate() error {
	if !ValidID(e.ID) || !namePattern.MatchString(e.Name) || len(e.Purpose) > 2048 || len(e.Fields) == 0 || len(e.Fields) > 64 {
		return ErrInvalid
	}
	if e.Expires != "" {
		if _, err := time.Parse("2006-01-02", e.Expires); err != nil {
			return ErrInvalid
		}
	}
	for k, f := range e.Fields {
		if !envPattern.MatchString(k) || len(f.Value) > 65536 || strings.ContainsRune(f.Value, 0) {
			return ErrInvalid
		}
	}
	return nil
}

// Heads validates the entire graph before computing the set of current versions.
func Heads(revs map[string]Revision) (map[string][]string, error) {
	if len(revs) > 2048 {
		return nil, ErrInvalid
	}
	children := map[string][]string{}
	degree := map[string]int{}
	head := map[string]bool{}
	for id, r := range revs {
		if id != r.ID || !ValidID(id) || !ValidID(r.Device) || r.Entry.Validate() != nil || len(r.Parents) > 2048 {
			return nil, ErrInvalid
		}
		head[id] = true
		seen := map[string]bool{}
		chosenIDSeen := false
		if r.Resolution && len(r.Parents) < 2 {
			return nil, ErrInvalid
		}
		for _, p := range r.Parents {
			parent, ok := revs[p]
			if !ok || p == id || seen[p] || (!r.Resolution && parent.Entry.ID != r.Entry.ID) || parent.Entry.Name != r.Entry.Name {
				return nil, ErrInvalid
			}
			if parent.Entry.ID == r.Entry.ID {
				chosenIDSeen = true
			}
			seen[p] = true
			children[p] = append(children[p], id)
		}
		if r.Resolution && !chosenIDSeen {
			return nil, ErrInvalid
		}
		degree[id] = len(r.Parents)
	}
	queue := []string{}
	for id, n := range degree {
		if n == 0 {
			queue = append(queue, id)
		}
	}
	visited := 0
	for len(queue) > 0 {
		id := queue[0]
		queue = queue[1:]
		visited++
		for _, c := range children[id] {
			head[id] = false
			degree[c]--
			if degree[c] == 0 {
				queue = append(queue, c)
			}
		}
	}
	if visited != len(revs) {
		return nil, ErrInvalid
	}
	result := map[string][]string{}
	for id := range revs {
		if head[id] {
			name := revs[id].Entry.Name
			result[name] = append(result[name], id)
		}
	}
	for name := range result {
		sort.Strings(result[name])
	}
	return result, nil
}

func Select(revs map[string]Revision, name string) (Entry, error) {
	heads, err := Heads(revs)
	if err != nil {
		return Entry{}, err
	}
	ids := heads[name]
	if len(ids) == 0 {
		return Entry{}, ErrNotFound
	}
	if len(ids) != 1 {
		return Entry{}, ErrConflict
	}
	e := revs[ids[0]].Entry
	if e.Disabled {
		return Entry{}, ErrDisabled
	}
	return e, nil
}

type Summary struct {
	Name    string   `json:"name"`
	Purpose string   `json:"purpose,omitempty"`
	State   string   `json:"state"`
	Fields  []string `json:"fields,omitempty"`
	Expires string   `json:"expires,omitempty"`
}

func Summaries(revs map[string]Revision) ([]Summary, error) {
	heads, err := Heads(revs)
	if err != nil {
		return nil, err
	}
	out := []Summary{}
	for name, ids := range heads {
		s := Summary{Name: name, State: "active"}
		if len(ids) != 1 {
			s.State = "conflict"
		} else {
			e := revs[ids[0]].Entry
			s.Purpose = e.Purpose
			s.Expires = e.Expires
			if e.Disabled {
				s.State = "disabled"
			}
			for k := range e.Fields {
				s.Fields = append(s.Fields, k)
			}
			sort.Strings(s.Fields)
		}
		out = append(out, s)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out, nil
}
