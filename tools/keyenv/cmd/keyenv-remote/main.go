// keyenv-remote is the human-side CLI for the remote keys service. It runs only
// on the owner's own computer: login, hidden-input set, and a one-time import
// from an unlocked local keyenv. Agents use the remote MCP server instead.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/remote"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/terminal"
)

var (
	namePattern  = regexp.MustCompile(`^[a-z][a-z0-9/_-]{0,79}$`)
	fieldPattern = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]{0,127}$`)
	datePattern  = regexp.MustCompile(`^\d{4}-\d{2}-\d{2}$`)
)

const usage = `keyenv-remote: write secrets to the remote keyenv store.

  keyenv-remote login [--server URL] [--scope "keys:write"] [--no-browser]
  keyenv-remote set NAME [--purpose TEXT] [--expires YYYY-MM-DD]
      Reads hidden K=V lines from the terminal; an empty line finishes.
  keyenv-remote import [--keyenv PATH] [--keyenv-home DIR] [--offline] [--update-existing] [NAME...]
      Copies entries from an unlocked local keyenv, once. Existing remote
      entries are skipped unless --update-existing is given.
  keyenv-remote logout

KEYENV_REMOTE_HOME overrides the private token directory.
Values and tokens are never printed.`

func out(v any) { json.NewEncoder(os.Stdout).Encode(v) }

func main() {
	if err := run(os.Args[1:]); err != nil {
		code := "ERROR"
		var tool *remote.ToolError
		switch {
		case errors.As(err, &tool):
			code = tool.Code
		case errors.Is(err, remote.ErrLoginRequired):
			code = "LOGIN_REQUIRED"
		case errors.Is(err, remote.ErrInvalid):
			code = "INVALID_ARGUMENT"
		case errors.Is(err, terminal.ErrInteraction):
			code = "INTERACTION_REQUIRED"
		}
		json.NewEncoder(os.Stderr).Encode(map[string]string{"error": code, "message": err.Error()})
		os.Exit(1)
	}
}

func run(args []string) error {
	if len(args) == 0 || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
		fmt.Println(usage)
		return nil
	}
	home, err := remote.DefaultHome()
	if err != nil {
		return err
	}
	client := remote.New(home)
	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Minute)
	defer cancel()
	command, rest := args[0], args[1:]
	f := flag.NewFlagSet(command, flag.ContinueOnError)
	f.SetOutput(io.Discard)
	switch command {
	case "login":
		server := f.String("server", remote.DefaultServer, "keys service origin")
		scope := f.String("scope", "keys:write", "requested scope")
		noBrowser := f.Bool("no-browser", false, "print the URL instead of opening a browser")
		if f.Parse(rest) != nil || f.NArg() != 0 {
			return remote.ErrInvalid
		}
		origin, err := remote.CheckServer(*server)
		if err != nil {
			return err
		}
		cred, err := client.Login(ctx, origin, *scope, func(u string) {
			fmt.Fprintln(os.Stderr, "Open this URL to sign in and choose what keyenv-remote may change:")
			fmt.Fprintln(os.Stderr, u)
			if !*noBrowser {
				openBrowser(u)
			}
		})
		if err != nil {
			return err
		}
		out(map[string]any{"result": "logged_in", "server": cred.Server, "scope": cred.Scope})
		return nil
	case "logout":
		if f.Parse(rest) != nil || f.NArg() != 0 {
			return remote.ErrInvalid
		}
		if err := os.Remove(home + "/credentials.json"); err != nil && !errors.Is(err, os.ErrNotExist) {
			return err
		}
		out(map[string]any{"result": "logged_out", "note": "Revoke the connection on the account page to end it server-side."})
		return nil
	case "set":
		purpose := f.String("purpose", "", "entry purpose")
		expires := f.String("expires", "", "YYYY-MM-DD; - clears")
		name, flags := splitName(rest)
		if f.Parse(flags) != nil || f.NArg() != 0 || !namePattern.MatchString(name) {
			return remote.ErrInvalid
		}
		if *expires != "" && *expires != "-" && !datePattern.MatchString(*expires) {
			return remote.ErrInvalid
		}
		values, err := readAssignments()
		if err != nil {
			return err
		}
		defer clearValues(values)
		set := map[string]bool{}
		f.Visit(func(fl *flag.Flag) { set[fl.Name] = true })
		meta := map[string]any{}
		if set["purpose"] {
			meta["purpose"] = *purpose
		}
		if set["expires"] {
			if *expires == "-" {
				meta["expires"] = nil
			} else {
				meta["expires"] = *expires
			}
		}
		result, err := upsert(ctx, client, name, values, meta, true)
		if err != nil {
			return err
		}
		out(result)
		return nil
	case "import":
		keyenvPath := f.String("keyenv", "keyenv", "local keyenv binary")
		keyenvHome := f.String("keyenv-home", "", "local keyenv --home")
		offline := f.Bool("offline", false, "read the verified local keyenv cache")
		update := f.Bool("update-existing", false, "overwrite fields of remote entries that already exist")
		if f.Parse(rest) != nil {
			return remote.ErrInvalid
		}
		return importAll(ctx, client, *keyenvPath, *keyenvHome, *offline, *update, f.Args())
	case "__import-entry":
		// Internal: started by `keyenv run`, which put the entry's fields in our environment.
		if len(rest) != 1 {
			return remote.ErrInvalid
		}
		return importEntry(ctx, client, rest[0])
	}
	return remote.ErrInvalid
}

// splitName lets NAME come before or after the flags.
func splitName(args []string) (string, []string) {
	if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		return args[0], args[1:]
	}
	if len(args) > 0 && !strings.HasPrefix(args[len(args)-1], "-") {
		return args[len(args)-1], args[:len(args)-1]
	}
	return "", args
}

func readAssignments() (map[string]string, error) {
	values := map[string]string{}
	fmt.Fprintln(os.Stderr, "Enter K=V lines (input hidden); an empty line finishes.")
	for {
		line, err := terminal.Read("K=V: ", true)
		if err != nil {
			clearValues(values)
			return nil, err
		}
		if len(line) == 0 {
			break
		}
		k, v, ok := strings.Cut(string(line), "=")
		clear(line)
		if !ok || !fieldPattern.MatchString(k) || strings.ContainsRune(v, 0) {
			fmt.Fprintln(os.Stderr, "Ignored: expected FIELD=value with an environment-variable field name.")
			continue
		}
		values[k] = v
		fmt.Fprintf(os.Stderr, "Recorded field %s.\n", k)
	}
	if len(values) == 0 {
		return nil, remote.ErrInvalid
	}
	return values, nil
}

// Go strings are immutable; dropping references is the best this process can do.
func clearValues(values map[string]string) {
	for k := range values {
		delete(values, k)
	}
}

func fieldNames(values map[string]string) []string {
	names := make([]string, 0, len(values))
	for k := range values {
		names = append(names, k)
	}
	sort.Strings(names)
	return names
}

type remoteEntry struct {
	exists, writable bool
}

func lookup(ctx context.Context, client *remote.Client, name string) (remoteEntry, error) {
	data, err := client.Call(ctx, "list_entries", map[string]any{})
	if err != nil {
		return remoteEntry{}, err
	}
	entries, _ := data["entries"].([]any)
	for _, e := range entries {
		m, _ := e.(map[string]any)
		if m["name"] == name {
			w, _ := m["writable"].(bool)
			return remoteEntry{exists: true, writable: w}, nil
		}
	}
	return remoteEntry{}, nil
}

// upsert creates the entry, or (when allowed) sets fields of an existing one.
// The server's reply contains names only; nothing here prints values.
func upsert(ctx context.Context, client *remote.Client, name string, values map[string]string, meta map[string]any, allowUpdate bool) (map[string]any, error) {
	current, err := lookup(ctx, client, name)
	if err != nil {
		return nil, err
	}
	if current.exists {
		if !allowUpdate {
			return map[string]any{"result": "skipped_existing", "name": name}, nil
		}
		if !current.writable {
			return nil, &remote.ToolError{Code: "WRITE_NOT_GRANTED", Message: "this login may not change " + name + "; log in again and allow changes"}
		}
		args := map[string]any{"name": name, "set_fields": values}
		for k, v := range meta {
			args[k] = v
		}
		data, err := client.Call(ctx, "update_entry", args)
		if err != nil {
			return nil, err
		}
		return map[string]any{"result": "updated", "name": data["name"], "fields": data["fields"]}, nil
	}
	args := map[string]any{"name": name, "fields": values}
	for k, v := range meta {
		if v != nil {
			args[k] = v
		}
	}
	data, err := client.Call(ctx, "create_entry", args)
	var tool *remote.ToolError
	if errors.As(err, &tool) && tool.Code == "ENTRY_EXISTS" {
		return nil, &remote.ToolError{Code: "ENTRY_EXISTS", Message: name + " exists but is not available to this login; log in again and allow changing it"}
	}
	if err != nil {
		return nil, err
	}
	return map[string]any{"result": "created", "name": data["name"], "fields": data["fields"]}, nil
}

type localSummary struct {
	Name    string   `json:"name"`
	Purpose string   `json:"purpose"`
	State   string   `json:"state"`
	Fields  []string `json:"fields"`
	Expires string   `json:"expires"`
}

func keyenvArgs(home string, args ...string) []string {
	if home != "" {
		return append([]string{"--home", home}, args...)
	}
	return args
}

func importAll(ctx context.Context, client *remote.Client, keyenvPath, keyenvHome string, offline, update bool, only []string) error {
	if _, err := client.Fresh(ctx, false); err != nil {
		return err
	}
	bin, err := exec.LookPath(keyenvPath)
	if err != nil {
		return fmt.Errorf("%w: keyenv not found", remote.ErrInvalid)
	}
	listArgs := []string{"list"}
	if offline {
		listArgs = append(listArgs, "--offline")
	}
	cmd := exec.CommandContext(ctx, bin, keyenvArgs(keyenvHome, listArgs...)...)
	cmd.Stderr = os.Stderr
	raw, err := cmd.Output()
	if err != nil {
		return fmt.Errorf("keyenv list failed (unlock keyenv first): %w", err)
	}
	var listed struct {
		Entries []localSummary `json:"entries"`
	}
	if json.Unmarshal(raw, &listed) != nil {
		return fmt.Errorf("%w: unexpected keyenv list output", remote.ErrServer)
	}
	self, err := os.Executable()
	if err != nil {
		return err
	}
	wanted := map[string]bool{}
	for _, n := range only {
		wanted[n] = true
	}
	results := []map[string]any{}
	for _, e := range listed.Entries {
		if len(wanted) > 0 && !wanted[e.Name] {
			continue
		}
		if e.State != "active" {
			results = append(results, map[string]any{"name": e.Name, "result": "skipped_" + e.State})
			continue
		}
		mode := "create"
		if update {
			mode = "update"
		}
		runArgs := []string{"run", "--credential", e.Name}
		if offline {
			runArgs = append(runArgs, "--offline")
		}
		runArgs = append(runArgs, "--", self, "__import-entry", e.Name)
		child := exec.CommandContext(ctx, bin, keyenvArgs(keyenvHome, runArgs...)...)
		child.Env = append(os.Environ(),
			"KEYENV_REMOTE_IMPORT_FIELDS="+strings.Join(e.Fields, ","),
			"KEYENV_REMOTE_IMPORT_PURPOSE="+e.Purpose,
			"KEYENV_REMOTE_IMPORT_EXPIRES="+e.Expires,
			"KEYENV_REMOTE_IMPORT_MODE="+mode,
		)
		var stderr strings.Builder
		child.Stderr = &stderr
		output, err := child.Output()
		var result map[string]any
		if err != nil || json.Unmarshal(lastLine(output), &result) != nil {
			result = map[string]any{"name": e.Name, "result": "failed", "detail": errorLine(stderr.String())}
		}
		results = append(results, result)
	}
	out(map[string]any{"result": "import_finished", "entries": results})
	return nil
}

func lastLine(b []byte) []byte {
	lines := strings.Split(strings.TrimSpace(string(b)), "\n")
	return []byte(lines[len(lines)-1])
}

// errorLine keeps only our own structured error from the child's stderr.
func errorLine(stderr string) string {
	for _, line := range strings.Split(stderr, "\n") {
		var e map[string]string
		if json.Unmarshal([]byte(line), &e) == nil && e["error"] != "" {
			return e["error"]
		}
	}
	return "child failed"
}

func importEntry(ctx context.Context, client *remote.Client, name string) error {
	if !namePattern.MatchString(name) {
		return remote.ErrInvalid
	}
	values := map[string]string{}
	for _, field := range strings.Split(os.Getenv("KEYENV_REMOTE_IMPORT_FIELDS"), ",") {
		if field == "" {
			continue
		}
		v, ok := os.LookupEnv(field)
		if !ok || !fieldPattern.MatchString(field) {
			return fmt.Errorf("%w: field %s missing from keyenv run", remote.ErrInvalid, field)
		}
		values[field] = v
	}
	defer clearValues(values)
	if len(values) == 0 {
		return remote.ErrInvalid
	}
	meta := map[string]any{}
	if p := os.Getenv("KEYENV_REMOTE_IMPORT_PURPOSE"); p != "" {
		meta["purpose"] = p
	}
	if e := os.Getenv("KEYENV_REMOTE_IMPORT_EXPIRES"); e != "" {
		meta["expires"] = e
	}
	result, err := upsert(ctx, client, name, values, meta, os.Getenv("KEYENV_REMOTE_IMPORT_MODE") == "update")
	if err != nil {
		return err
	}
	if result["fields"] == nil {
		result["fields"] = fieldNames(values)
	}
	out(result)
	return nil
}

func openBrowser(u string) {
	var cmd *exec.Cmd
	switch runtime.GOOS {
	case "darwin":
		cmd = exec.Command("open", u)
	case "linux":
		cmd = exec.Command("xdg-open", u)
	default:
		return
	}
	cmd.Stdout, cmd.Stderr = nil, nil
	_ = cmd.Start()
}
