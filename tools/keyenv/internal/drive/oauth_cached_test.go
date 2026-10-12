package drive

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"golang.org/x/oauth2"
)

type testCache struct {
	token  *oauth2.Token
	writes int
	fail   error
}

func (c *testCache) Load() (*oauth2.Token, error) { return c.token, nil }
func (c *testCache) Save(t *oauth2.Token) error {
	if c.fail != nil {
		return c.fail
	}
	cp := *t
	c.token = &cp
	c.writes++
	return nil
}
func expiredTestToken() *oauth2.Token {
	return &oauth2.Token{AccessToken: "SYNTHETIC-old", RefreshToken: "SYNTHETIC-refresh", TokenType: "Bearer", Expiry: time.Now().Add(-time.Hour)}
}
func TestRefreshRotationSurvivesNewTokenSourceWithoutBrowser(t *testing.T) {
	var calls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_ = r.ParseForm()
		if r.Form.Get("grant_type") != "refresh_token" {
			t.Error("unexpected grant")
		}
		expected := "SYNTHETIC-refresh"
		if calls.Add(1) > 1 {
			expected = "SYNTHETIC-rotated"
		}
		if r.Form.Get("refresh_token") != expected {
			t.Error("refresh rotation was lost")
		}
		w.Header().Set("Content-Type", "application/json")
		fmt.Fprintf(w, `{"access_token":"SYNTHETIC-new","refresh_token":"SYNTHETIC-rotated","token_type":"Bearer","expires_in":3600,"scope":%q}`, DriveFileScope)
	}))
	defer server.Close()
	cfg := oauth2.Config{ClientID: "test-client", Endpoint: oauth2.Endpoint{TokenURL: server.URL, AuthStyle: oauth2.AuthStyleInParams}, Scopes: []string{DriveFileScope}}
	initial := expiredTestToken()
	cache := &testCache{token: initial}
	source := newSavingSource(cfg.TokenSource(context.Background(), initial), cache, initial)
	var group sync.WaitGroup
	for range 8 {
		group.Go(func() {
			if _, err := source.Token(); err != nil {
				t.Error(err)
			}
		})
	}
	group.Wait()
	if calls.Load() != 1 || cache.writes != 1 || cache.token.RefreshToken != "SYNTHETIC-rotated" {
		t.Fatal("refresh not serialized and persisted")
	}
	next := *cache.token
	next.Expiry = time.Now().Add(-time.Minute)
	reopened := newSavingSource(cfg.TokenSource(context.Background(), &next), cache, &next)
	if _, err := reopened.Token(); err != nil || calls.Load() != 2 {
		t.Fatal("subsequent source could not refresh", err)
	}
}
func TestRefreshFailuresDoNotExposeBodiesOrOverwriteGrant(t *testing.T) {
	for _, kind := range []string{"revoked", "server", "scope", "save"} {
		t.Run(kind, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
				w.Header().Set("Content-Type", "application/json")
				if kind == "revoked" {
					w.WriteHeader(400)
					io.WriteString(w, `{"error":"invalid_grant","error_description":"SYNTHETIC-secret-must-not-leak"}`)
					return
				}
				if kind == "server" {
					w.WriteHeader(503)
					io.WriteString(w, "SYNTHETIC-secret-must-not-leak")
					return
				}
				scope := DriveFileScope
				if kind == "scope" {
					scope = "https://www.googleapis.com/auth/drive"
				}
				fmt.Fprintf(w, `{"access_token":"SYNTHETIC-new","token_type":"Bearer","expires_in":3600,"scope":%q}`, scope)
			}))
			defer server.Close()
			initial := expiredTestToken()
			cache := &testCache{token: initial}
			want := vault.ErrAuthRequired
			if kind == "server" {
				want = vault.ErrUnavailable
			}
			if kind == "save" {
				cache.fail = errors.New("SYNTHETIC-storage-error")
				want = cache.fail
			}
			cfg := oauth2.Config{ClientID: "test-client", Endpoint: oauth2.Endpoint{TokenURL: server.URL, AuthStyle: oauth2.AuthStyleInParams}}
			source := newSavingSource(cfg.TokenSource(context.Background(), initial), cache, initial)
			_, err := source.Token()
			if !errors.Is(err, want) || cache.writes != 0 || cache.token != initial {
				t.Fatal("failure exposed a body or overwrote the grant", err)
			}
		})
	}
}

type roundTripFunc func(*http.Request) (*http.Response, error)

func (f roundTripFunc) RoundTrip(r *http.Request) (*http.Response, error) { return f(r) }

type consentWriter struct{ lines chan string }

func (w consentWriter) Write(p []byte) (int, error) { w.lines <- string(p); return len(p), nil }
func TestFirstConsentIsOfflineAndWrongAccountNeverPersists(t *testing.T) {
	for _, wrong := range []bool{false, true} {
		t.Run(fmt.Sprint(wrong), func(t *testing.T) {
			root := t.TempDir()
			clientPath := filepath.Join(root, "client.json")
			if err := os.WriteFile(clientPath, []byte(`{"installed":{"client_id":"SYNTHETIC-existing-client","client_secret":"SYNTHETIC-client-secret"}}`), 0600); err != nil {
				t.Fatal(err)
			}
			transport := roundTripFunc(func(r *http.Request) (*http.Response, error) {
				body := ""
				switch r.URL.Host {
				case "oauth2.googleapis.com":
					_ = r.ParseForm()
					clientID := r.Form.Get("client_id")
					if clientID == "" {
						clientID, _, _ = r.BasicAuth()
					}
					if clientID != "SYNTHETIC-existing-client" || r.Form.Get("code_verifier") == "" {
						t.Error("existing client or PKCE missing")
					}
					body = fmt.Sprintf(`{"access_token":"SYNTHETIC-access","refresh_token":"SYNTHETIC-refresh","token_type":"Bearer","expires_in":3600,"scope":%q}`, DriveFileScope)
				case "www.googleapis.com":
					if r.Header.Get("Authorization") != "Bearer SYNTHETIC-access" {
						t.Error("access token missing")
					}
					account := "person@example.invalid"
					if wrong {
						account = "other@example.invalid"
					}
					body = fmt.Sprintf(`{"user":{"emailAddress":%q}}`, account)
				default:
					t.Error("unexpected host")
					return nil, errors.New("refused")
				}
				return &http.Response{StatusCode: 200, Header: http.Header{"Content-Type": []string{"application/json"}}, Body: io.NopCloser(strings.NewReader(body)), Request: r}, nil
			})
			ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
			defer cancel()
			ctx = context.WithValue(ctx, oauth2.HTTPClient, &http.Client{Transport: transport})
			lines := make(chan string, 4)
			cache := &testCache{}
			done := make(chan error, 1)
			go func() {
				_, err := AuthorizeCached(ctx, clientPath, "person@example.invalid", consentWriter{lines}, cache)
				done <- err
			}()
			var authURL *url.URL
			for authURL == nil {
				select {
				case line := <-lines:
					if strings.HasPrefix(line, "https://accounts.google.com/") {
						authURL, _ = url.Parse(strings.TrimSpace(line))
					}
				case <-ctx.Done():
					t.Fatal("authorization instructions missing")
				}
			}
			q := authURL.Query()
			if q.Get("access_type") != "offline" || q.Get("scope") != DriveFileScope || q.Get("prompt") != "consent select_account" || q.Get("client_id") != "SYNTHETIC-existing-client" || q.Get("code_challenge_method") != "S256" {
				t.Fatal("unexpected authorization options")
			}
			callback, _ := url.Parse(q.Get("redirect_uri"))
			params := url.Values{"state": []string{q.Get("state")}, "code": []string{"SYNTHETIC-code"}}
			callback.RawQuery = params.Encode()
			response, err := http.Get(callback.String())
			if err != nil {
				t.Fatal("synthetic callback failed")
			}
			response.Body.Close()
			err = <-done
			if wrong {
				if !errors.Is(err, vault.ErrAuthRequired) || cache.writes != 0 {
					t.Fatal("wrong account cached", err)
				}
			} else {
				if err != nil || cache.writes != 1 {
					t.Fatal("grant not saved", err)
				}
			}
		})
	}
}
func TestTokenEndpointCannotRedirectRefreshCredentials(t *testing.T) {
	var leaked atomic.Int32
	destination := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { leaked.Add(1) }))
	defer destination.Close()
	endpoint := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, destination.URL, http.StatusTemporaryRedirect)
	}))
	defer endpoint.Close()
	initial := expiredTestToken()
	cache := &testCache{token: initial}
	cfg := oauth2.Config{ClientID: "test-client", Endpoint: oauth2.Endpoint{TokenURL: endpoint.URL, AuthStyle: oauth2.AuthStyleInParams}}
	source := newSavingSource(cfg.TokenSource(secureOAuthContext(context.Background()), initial), cache, initial)
	if _, err := source.Token(); !errors.Is(err, vault.ErrUnavailable) || leaked.Load() != 0 || cache.writes != 0 {
		t.Fatal("token redirect forwarded credentials", err)
	}
}
