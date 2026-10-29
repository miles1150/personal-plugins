package remote

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
	"time"
)

// fakeServer is a minimal keys service: discovery, authorization redirect,
// token endpoint with refresh rotation, and an MCP endpoint.
type fakeServer struct {
	*httptest.Server
	mu       sync.Mutex
	issuer   string // overrides the iss returned to the callback when set
	refresh  string
	access   string
	rotated  int
	lastTool string
	lastArgs map[string]any
	cimd     bool
	dcr      bool
}

func newFake(t *testing.T) *fakeServer {
	f := &fakeServer{refresh: "r0", access: "a0", cimd: true}
	mux := http.NewServeMux()
	f.Server = httptest.NewServer(mux)
	t.Cleanup(f.Close)
	origin := f.URL
	mux.HandleFunc("/.well-known/oauth-protected-resource/mcp", func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{"resource": origin + "/mcp", "authorization_servers": []string{origin}})
	})
	mux.HandleFunc("/.well-known/oauth-authorization-server", func(w http.ResponseWriter, r *http.Request) {
		meta := map[string]any{"issuer": origin, "authorization_endpoint": origin + "/oauth/authorize", "token_endpoint": origin + "/oauth/token",
			"client_id_metadata_document_supported": f.cimd, "authorization_response_iss_parameter_supported": true,
			"token_endpoint_auth_methods_supported": []string{"none"}, "code_challenge_methods_supported": []string{"S256"}}
		if f.dcr {
			meta["registration_endpoint"] = origin + "/oauth/register"
		}
		json.NewEncoder(w).Encode(meta)
	})
	mux.HandleFunc("/oauth/register", func(w http.ResponseWriter, r *http.Request) {
		var body struct {
			RedirectURIs []string `json:"redirect_uris"`
			AuthMethod   string   `json:"token_endpoint_auth_method"`
		}
		if r.Method != http.MethodPost || json.NewDecoder(r.Body).Decode(&body) != nil || len(body.RedirectURIs) != 1 || !strings.HasPrefix(body.RedirectURIs[0], "http://127.0.0.1:") || body.AuthMethod != "none" {
			w.WriteHeader(http.StatusBadRequest)
			return
		}
		w.WriteHeader(http.StatusCreated)
		json.NewEncoder(w).Encode(map[string]any{"client_id": "dcr-test", "token_endpoint_auth_method": "none"})
	})
	mux.HandleFunc("/oauth/token", func(w http.ResponseWriter, r *http.Request) {
		r.ParseForm()
		f.mu.Lock()
		defer f.mu.Unlock()
		if r.Form.Get("client_id") != origin+DocumentPath && r.Form.Get("client_id") != "dcr-test" || r.Form.Get("resource") != origin+"/mcp" {
			w.WriteHeader(400)
			json.NewEncoder(w).Encode(map[string]string{"error": "invalid_client"})
			return
		}
		switch r.Form.Get("grant_type") {
		case "authorization_code":
			if r.Form.Get("code") != "the-code" || r.Form.Get("code_verifier") == "" || !strings.HasPrefix(r.Form.Get("redirect_uri"), "http://127.0.0.1:") {
				w.WriteHeader(400)
				json.NewEncoder(w).Encode(map[string]string{"error": "invalid_grant"})
				return
			}
		case "refresh_token":
			if r.Form.Get("refresh_token") != f.refresh {
				w.WriteHeader(400)
				json.NewEncoder(w).Encode(map[string]string{"error": "invalid_grant"})
				return
			}
			f.rotated++
		}
		f.refresh = "r" + string(rune('0'+f.rotated+1))
		f.access = "a" + string(rune('0'+f.rotated+1))
		json.NewEncoder(w).Encode(map[string]any{"access_token": f.access, "refresh_token": f.refresh, "expires_in": 300, "scope": "keys:write"})
	})
	mux.HandleFunc("/mcp", func(w http.ResponseWriter, r *http.Request) {
		f.mu.Lock()
		defer f.mu.Unlock()
		if r.Header.Get("authorization") != "Bearer "+f.access || r.Header.Get("mcp-protocol-version") == "" {
			w.WriteHeader(401)
			return
		}
		var req struct {
			Params struct {
				Name      string         `json:"name"`
				Arguments map[string]any `json:"arguments"`
			} `json:"params"`
		}
		json.NewDecoder(r.Body).Decode(&req)
		f.lastTool, f.lastArgs = req.Params.Name, req.Params.Arguments
		var envelope map[string]any
		switch req.Params.Name {
		case "list_entries":
			envelope = map[string]any{"ok": true, "data": map[string]any{"entries": []any{map[string]any{"name": "old/entry", "writable": true}}, "can_create": true}}
		case "create_entry", "update_entry":
			envelope = map[string]any{"ok": true, "data": map[string]any{"name": req.Params.Arguments["name"], "fields": []string{"TOKEN"}}}
		default:
			envelope = map[string]any{"ok": false, "error": map[string]any{"code": "NOPE", "message": "unknown"}}
		}
		json.NewEncoder(w).Encode(map[string]any{"jsonrpc": "2.0", "id": 1, "result": map[string]any{"structuredContent": envelope}})
	})
	return f
}

// browser follows the authorization URL like a user who approves.
func (f *fakeServer) browser(t *testing.T, state *string) func(string) {
	return func(authURL string) {
		u, err := url.Parse(authURL)
		if err != nil {
			t.Error(err)
			return
		}
		q := u.Query()
		if q.Get("client_id") != f.URL+DocumentPath && q.Get("client_id") != "dcr-test" || q.Get("code_challenge_method") != "S256" || q.Get("resource") != f.URL+"/mcp" {
			t.Errorf("unexpected authorization request %v", q)
		}
		iss := f.URL
		if f.issuer != "" {
			iss = f.issuer
		}
		if state != nil {
			*state = q.Get("state")
		}
		cb := q.Get("redirect_uri") + "?" + url.Values{"code": {"the-code"}, "state": {q.Get("state")}, "iss": {iss}}.Encode()
		go func() {
			resp, err := http.Get(cb)
			if err == nil {
				resp.Body.Close()
			}
		}()
	}
}

func TestLoginFallsBackToDCR(t *testing.T) {
	f := newFake(t)
	f.cimd, f.dcr = false, true
	c := New(t.TempDir())
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	cred, err := c.Login(ctx, f.URL, "keys:write", f.browser(t, nil))
	if err != nil || cred.ClientID != "dcr-test" {
		t.Fatalf("DCR login: %v", err)
	}
	saved, err := c.Load()
	if err != nil || saved.ClientID != "dcr-test" {
		t.Fatalf("DCR credentials not saved: %v", err)
	}
	c.Now = func() time.Time { return time.Now().Add(time.Hour) }
	if _, err := c.Fresh(ctx, false); err != nil {
		t.Fatalf("DCR refresh: %v", err)
	}
}

func TestLoginStoresPrivateCredentialsAndRefreshRotates(t *testing.T) {
	f := newFake(t)
	home := filepath.Join(t.TempDir(), "home")
	c := New(home)
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	server, err := CheckServer(f.URL)
	if err != nil {
		t.Fatal(err)
	}
	cred, err := c.Login(ctx, server, "keys:write", f.browser(t, nil))
	if err != nil {
		t.Fatal(err)
	}
	if cred.ClientID != f.URL+DocumentPath || cred.RefreshToken != "r1" {
		t.Fatalf("unexpected credentials %+v", cred)
	}
	info, err := os.Stat(filepath.Join(home, "credentials.json"))
	if err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("credentials mode %v %v", info, err)
	}
	dir, _ := os.Stat(home)
	if dir.Mode().Perm() != 0o700 {
		t.Fatalf("home mode %v", dir.Mode())
	}
	// An expired access token is refreshed and the rotated pair saved first.
	c.Now = func() time.Time { return time.Now().Add(time.Hour) }
	data, err := c.Call(ctx, "create_entry", map[string]any{"name": "x/y", "fields": map[string]string{"TOKEN": "value"}})
	if err != nil || data["name"] != "x/y" {
		t.Fatalf("call: %v %v", data, err)
	}
	saved, _ := c.Load()
	if saved.RefreshToken != "r2" || f.rotated != 1 {
		t.Fatalf("refresh not rotated and saved: %+v rotated=%d", saved, f.rotated)
	}
	// A revoked refresh token means: log in again.
	f.mu.Lock()
	f.refresh = "revoked"
	f.mu.Unlock()
	if _, err := c.Fresh(ctx, true); !errors.Is(err, ErrLoginRequired) {
		t.Fatalf("expected LOGIN_REQUIRED, got %v", err)
	}
}

func TestLoginRejectsWrongIssuerAndUnsupportedServer(t *testing.T) {
	f := newFake(t)
	f.issuer = "https://evil.example"
	c := New(t.TempDir())
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	if _, err := c.Login(ctx, f.URL, "keys:write", f.browser(t, nil)); err == nil || !strings.Contains(err.Error(), "issuer mismatch") {
		t.Fatalf("expected issuer mismatch, got %v", err)
	}
	f.issuer = ""
	f.cimd = false
	if _, err := c.Login(ctx, f.URL, "keys:write", f.browser(t, nil)); err == nil {
		t.Fatal("login must require metadata-document support")
	}
	if _, err := os.Stat(filepath.Join(c.Home, "credentials.json")); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("failed login must not write credentials")
	}
}

func TestCheckServer(t *testing.T) {
	for _, ok := range []string{"https://keys.miles1150.top", "https://keys.miles1150.top/", "http://127.0.0.1:8795"} {
		if _, err := CheckServer(ok); err != nil {
			t.Errorf("%s: %v", ok, err)
		}
	}
	for _, bad := range []string{"http://keys.miles1150.top", "https://keys.miles1150.top/mcp", "https://u@keys.example", "https://keys.example?x", "ftp://x"} {
		if _, err := CheckServer(bad); err == nil {
			t.Errorf("%s accepted", bad)
		}
	}
}

func TestUnsafeCredentialFileIsRefused(t *testing.T) {
	home := t.TempDir()
	c := New(home)
	if err := c.Save(&Credentials{RefreshToken: "r"}); err != nil {
		t.Fatal(err)
	}
	os.Chmod(filepath.Join(home, "credentials.json"), 0o644)
	if _, err := c.Load(); err == nil {
		t.Fatal("world-readable credentials must be refused")
	}
}
