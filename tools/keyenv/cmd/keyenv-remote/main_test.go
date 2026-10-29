package main

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/remote"
)

// The test binary doubles as keyenv-remote for the `keyenv run` child.
func TestMain(m *testing.M) {
	if len(os.Args) > 1 && os.Args[1] == "__import-entry" {
		if err := run(os.Args[1:]); err != nil {
			json.NewEncoder(os.Stderr).Encode(map[string]string{"error": err.Error()})
			os.Exit(1)
		}
		os.Exit(0)
	}
	os.Exit(m.Run())
}

// fakeKeyenv mimics `keyenv list` and `keyenv run --credential NAME -- CMD`:
// the child receives the entry's fields as environment variables.
const fakeKeyenv = `#!/bin/sh
if [ "$1" = "list" ]; then
  echo '{"unlocked":true,"entries":[{"name":"github/token","purpose":"CI","state":"active","fields":["GH_TOKEN"],"expires":"2999-01-01"},{"name":"old/entry","state":"active","fields":["A"]},{"name":"gone","state":"disabled"}]}'
  exit 0
fi
if [ "$1" = "run" ]; then
  name="$3"; shift 3
  [ "$1" = "--offline" ] && shift
  shift
  case "$name" in
    github/token) GH_TOKEN='s3cret$value with spaces' exec "$@" ;;
    old/entry) A='other' exec "$@" ;;
  esac
fi
exit 2
`

type mcpServer struct {
	mu    sync.Mutex
	calls []map[string]any
	raw   []string
}

func TestImportUsesKeyenvRunAndNeverPrintsValues(t *testing.T) {
	var s mcpServer
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var req struct {
			Params struct {
				Name      string         `json:"name"`
				Arguments map[string]any `json:"arguments"`
			} `json:"params"`
		}
		json.NewDecoder(r.Body).Decode(&req)
		s.mu.Lock()
		s.calls = append(s.calls, map[string]any{"tool": req.Params.Name, "args": req.Params.Arguments})
		s.mu.Unlock()
		data := map[string]any{"entries": []any{map[string]any{"name": "old/entry", "writable": true}}}
		if req.Params.Name == "create_entry" {
			data = map[string]any{"name": req.Params.Arguments["name"], "fields": []string{"GH_TOKEN"}}
		}
		json.NewEncoder(w).Encode(map[string]any{"result": map[string]any{"structuredContent": map[string]any{"ok": true, "data": data}}})
	}))
	defer srv.Close()
	home := t.TempDir()
	t.Setenv("KEYENV_REMOTE_HOME", home)
	c := remote.New(home)
	c.Save(&remote.Credentials{Server: srv.URL, Resource: srv.URL + "/mcp", TokenURL: srv.URL + "/token", AccessToken: "a", RefreshToken: "r", ExpiresAt: time.Now().Add(time.Hour).Unix()})
	bin := filepath.Join(t.TempDir(), "keyenv")
	os.WriteFile(bin, []byte(fakeKeyenv), 0o755)

	stdout := captureStdout(t, func() {
		if err := importAll(context.Background(), c, bin, "", false, false, nil); err != nil {
			t.Fatal(err)
		}
	})
	if strings.Contains(stdout, "s3cret") {
		t.Fatal("import output contains a value")
	}
	var report struct {
		Entries []map[string]any `json:"entries"`
	}
	if err := json.Unmarshal([]byte(stdout), &report); err != nil {
		t.Fatalf("%v: %s", err, stdout)
	}
	got := map[string]string{}
	for _, e := range report.Entries {
		got[e["name"].(string)] = e["result"].(string)
	}
	want := map[string]string{"github/token": "created", "old/entry": "skipped_existing", "gone": "skipped_disabled"}
	for k, v := range want {
		if got[k] != v {
			t.Fatalf("%s: got %q want %q (%v)", k, got[k], v, report.Entries)
		}
	}
	// The value reached the server exactly, with purpose and expiry preserved.
	var create map[string]any
	for _, call := range s.calls {
		if call["tool"] == "create_entry" {
			create = call["args"].(map[string]any)
		}
	}
	fields := create["fields"].(map[string]any)
	if fields["GH_TOKEN"] != "s3cret$value with spaces" || create["purpose"] != "CI" || create["expires"] != "2999-01-01" {
		t.Fatalf("unexpected create arguments: purpose=%v expires=%v", create["purpose"], create["expires"])
	}
}

func captureStdout(t *testing.T, f func()) string {
	t.Helper()
	r, w, _ := os.Pipe()
	old := os.Stdout
	os.Stdout = w
	f()
	w.Close()
	os.Stdout = old
	b := make([]byte, 1<<16)
	n, _ := r.Read(b)
	return string(b[:n])
}

func TestSplitName(t *testing.T) {
	for _, args := range [][]string{{"github/token", "--purpose", "x"}, {"--purpose", "x", "github/token"}} {
		name, _ := splitName(args)
		if name != "github/token" {
			t.Fatalf("%v -> %q", args, name)
		}
	}
}

func TestImportReportsFailureAndMissingSelection(t *testing.T) {
	home := t.TempDir()
	c := remote.New(home)
	if err := c.Save(&remote.Credentials{RefreshToken: "r", AccessToken: "a", ExpiresAt: time.Now().Add(time.Hour).Unix()}); err != nil {
		t.Fatal(err)
	}
	bin := filepath.Join(t.TempDir(), "keyenv")
	if err := os.WriteFile(bin, []byte(fakeKeyenv), 0o755); err != nil {
		t.Fatal(err)
	}
	var importErr error
	stdout := captureStdout(t, func() {
		importErr = importAll(context.Background(), c, bin, "", false, false, []string{"github/token", "absent"})
	})
	if importErr == nil || !strings.Contains(stdout, `"result":"failed"`) || !strings.Contains(stdout, `"result":"not_found"`) {
		t.Fatalf("expected incomplete import and entry results, got error %v and report %s", importErr, stdout)
	}
	if strings.Contains(stdout, `"old/entry"`) || strings.Contains(stdout, "s3cret") {
		t.Fatal("report included an unselected entry or secret")
	}
}
