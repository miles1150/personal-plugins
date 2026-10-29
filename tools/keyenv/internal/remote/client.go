// Package remote is the keyenv-remote client: OAuth login against the keys
// service with its own Client ID Metadata Document, a private token store, and
// MCP tool calls. It never prints or logs secret values or tokens.
package remote

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	DefaultServer   = "https://keys.miles1150.top"
	DocumentPath    = "/oauth/clients/keyenv-remote.json"
	ProtocolVersion = "2025-11-25"
	maxResponse     = 1 << 20
)

var (
	ErrLoginRequired = errors.New("LOGIN_REQUIRED")
	ErrServer        = errors.New("SERVER_REJECTED")
	ErrInvalid       = errors.New("INVALID_ARGUMENT")
)

// Credentials is the private token file. Only this process family reads it.
type Credentials struct {
	Server       string `json:"server"`
	Issuer       string `json:"issuer"`
	ClientID     string `json:"client_id"`
	Resource     string `json:"resource"`
	TokenURL     string `json:"token_endpoint"`
	Scope        string `json:"scope"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresAt    int64  `json:"expires_at"`
}

type Client struct {
	Home string
	HTTP *http.Client
	Now  func() time.Time
}

func New(home string) *Client {
	return &Client{
		Home: home,
		// No redirects: every endpoint is discovered and checked explicitly.
		HTTP: &http.Client{Timeout: 30 * time.Second, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }},
		Now:  time.Now,
	}
}

// DefaultHome is ~/.config/keyenv-remote (or the platform equivalent).
func DefaultHome() (string, error) {
	if h := os.Getenv("KEYENV_REMOTE_HOME"); h != "" {
		return h, nil
	}
	dir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "keyenv-remote"), nil
}

// CheckServer accepts HTTPS origins, and literal-loopback HTTP for local testing.
func CheckServer(raw string) (string, error) {
	u, err := url.Parse(raw)
	if err != nil || u.Host == "" || u.User != nil || (u.Path != "" && u.Path != "/") || u.RawQuery != "" || u.Fragment != "" {
		return "", ErrInvalid
	}
	if u.Scheme != "https" && !(u.Scheme == "http" && (u.Hostname() == "127.0.0.1" || u.Hostname() == "::1")) {
		return "", ErrInvalid
	}
	return u.Scheme + "://" + u.Host, nil
}

func (c *Client) path() string { return filepath.Join(c.Home, "credentials.json") }

func (c *Client) Load() (*Credentials, error) {
	info, err := os.Lstat(c.path())
	if errors.Is(err, os.ErrNotExist) {
		return nil, ErrLoginRequired
	}
	if err != nil || !info.Mode().IsRegular() || info.Mode().Perm()&0o077 != 0 {
		return nil, errors.New("CREDENTIALS_UNSAFE")
	}
	data, err := os.ReadFile(c.path())
	if err != nil {
		return nil, err
	}
	var cred Credentials
	if json.Unmarshal(data, &cred) != nil || cred.RefreshToken == "" {
		return nil, ErrLoginRequired
	}
	return &cred, nil
}

// Save writes the token file atomically with mode 0600 in a 0700 directory.
func (c *Client) Save(cred *Credentials) error {
	if err := os.MkdirAll(c.Home, 0o700); err != nil {
		return err
	}
	if err := os.Chmod(c.Home, 0o700); err != nil {
		return err
	}
	data, err := json.Marshal(cred)
	if err != nil {
		return err
	}
	tmp, err := os.CreateTemp(c.Home, ".credentials-*")
	if err != nil {
		return err
	}
	defer os.Remove(tmp.Name())
	if err = tmp.Chmod(0o600); err == nil {
		if _, err = tmp.Write(data); err == nil {
			err = tmp.Sync()
		}
	}
	if cerr := tmp.Close(); err == nil {
		err = cerr
	}
	if err != nil {
		return err
	}
	return os.Rename(tmp.Name(), c.path())
}

func (c *Client) getJSON(ctx context.Context, target string, out any) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, target, nil)
	if err != nil {
		return err
	}
	req.Header.Set("accept", "application/json")
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("%w: %s returned %d", ErrServer, target, resp.StatusCode)
	}
	return json.NewDecoder(io.LimitReader(resp.Body, maxResponse)).Decode(out)
}

type serverMetadata struct {
	Issuer                string   `json:"issuer"`
	AuthorizationEndpoint string   `json:"authorization_endpoint"`
	TokenEndpoint         string   `json:"token_endpoint"`
	RegistrationEndpoint  string   `json:"registration_endpoint"`
	CIMD                  bool     `json:"client_id_metadata_document_supported"`
	IssuerResponse        bool     `json:"authorization_response_iss_parameter_supported"`
	AuthMethods           []string `json:"token_endpoint_auth_methods_supported"`
	Challenges            []string `json:"code_challenge_methods_supported"`
}

func contains(list []string, v string) bool {
	for _, x := range list {
		if x == v {
			return true
		}
	}
	return false
}

// Discover follows RFC 9728 then RFC 8414 and checks the profile this client needs.
func (c *Client) Discover(ctx context.Context, server string) (*Credentials, *serverMetadata, error) {
	var resource struct {
		Resource string   `json:"resource"`
		Servers  []string `json:"authorization_servers"`
	}
	if err := c.getJSON(ctx, server+"/.well-known/oauth-protected-resource/mcp", &resource); err != nil {
		return nil, nil, err
	}
	if resource.Resource != server+"/mcp" || len(resource.Servers) != 1 {
		return nil, nil, fmt.Errorf("%w: unexpected protected resource metadata", ErrServer)
	}
	issuer := resource.Servers[0]
	if _, err := CheckServer(issuer); err != nil {
		return nil, nil, fmt.Errorf("%w: unexpected authorization server", ErrServer)
	}
	var meta serverMetadata
	if err := c.getJSON(ctx, issuer+"/.well-known/oauth-authorization-server", &meta); err != nil {
		return nil, nil, err
	}
	if meta.Issuer != issuer || (!meta.CIMD && !strings.HasPrefix(meta.RegistrationEndpoint, issuer+"/")) || !meta.IssuerResponse || !contains(meta.AuthMethods, "none") || !contains(meta.Challenges, "S256") ||
		!strings.HasPrefix(meta.AuthorizationEndpoint, issuer+"/") || !strings.HasPrefix(meta.TokenEndpoint, issuer+"/") {
		return nil, nil, fmt.Errorf("%w: authorization server does not support this client", ErrServer)
	}
	return &Credentials{Server: server, Issuer: issuer, ClientID: server + DocumentPath, Resource: resource.Resource, TokenURL: meta.TokenEndpoint}, &meta, nil
}

// register requests a public client with the exact loopback callback for this login.
func (c *Client) register(ctx context.Context, endpoint, redirect string) (string, error) {
	body, err := json.Marshal(map[string]any{
		"client_name": "keyenv-remote", "redirect_uris": []string{redirect},
		"token_endpoint_auth_method": "none", "grant_types": []string{"authorization_code", "refresh_token"},
		"response_types": []string{"code"},
	})
	if err != nil {
		return "", err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, endpoint, bytes.NewReader(body))
	if err != nil {
		return "", err
	}
	req.Header.Set("content-type", "application/json")
	req.Header.Set("accept", "application/json")
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()
	var registered struct {
		ClientID     string `json:"client_id"`
		ClientSecret string `json:"client_secret"`
		AuthMethod   string `json:"token_endpoint_auth_method"`
	}
	if resp.StatusCode != http.StatusCreated || json.NewDecoder(io.LimitReader(resp.Body, maxResponse)).Decode(&registered) != nil || registered.ClientID == "" || registered.ClientSecret != "" || registered.AuthMethod != "none" {
		return "", fmt.Errorf("%w: invalid client registration response", ErrServer)
	}
	return registered.ClientID, nil
}

func random() string {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return base64.RawURLEncoding.EncodeToString(b)
}

type tokenResponse struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	Scope        string `json:"scope"`
	Error        string `json:"error"`
}

func (c *Client) token(ctx context.Context, cred *Credentials, form url.Values) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, cred.TokenURL, strings.NewReader(form.Encode()))
	if err != nil {
		return err
	}
	req.Header.Set("content-type", "application/x-www-form-urlencoded")
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	var t tokenResponse
	if json.NewDecoder(io.LimitReader(resp.Body, maxResponse)).Decode(&t) != nil {
		return fmt.Errorf("%w: token endpoint returned %d", ErrServer, resp.StatusCode)
	}
	if resp.StatusCode != http.StatusOK || t.AccessToken == "" || t.RefreshToken == "" {
		if t.Error == "invalid_grant" {
			return ErrLoginRequired
		}
		return fmt.Errorf("%w: token endpoint error %s", ErrServer, t.Error)
	}
	cred.AccessToken, cred.RefreshToken, cred.Scope = t.AccessToken, t.RefreshToken, t.Scope
	cred.ExpiresAt = c.Now().Unix() + t.ExpiresIn
	return nil
}

// Login runs an authorization-code + PKCE flow on a loopback callback.
// openURL receives the authorization URL (print it, or open a browser).
func (c *Client) Login(ctx context.Context, server, scope string, openURL func(string)) (*Credentials, error) {
	cred, meta, err := c.Discover(ctx, server)
	if err != nil {
		return nil, err
	}
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		return nil, err
	}
	defer listener.Close()
	redirect := fmt.Sprintf("http://127.0.0.1:%d/callback", listener.Addr().(*net.TCPAddr).Port)
	if !meta.CIMD {
		cred.ClientID, err = c.register(ctx, meta.RegistrationEndpoint, redirect)
		if err != nil {
			return nil, err
		}
	}
	state, verifier := random(), random()
	sum := sha256.Sum256([]byte(verifier))
	query := url.Values{
		"response_type":         {"code"},
		"client_id":             {cred.ClientID},
		"redirect_uri":          {redirect},
		"scope":                 {scope},
		"state":                 {state},
		"code_challenge":        {base64.RawURLEncoding.EncodeToString(sum[:])},
		"code_challenge_method": {"S256"},
		"resource":              {cred.Resource},
	}
	type result struct {
		code string
		err  error
	}
	done := make(chan result, 1)
	mux := http.NewServeMux()
	mux.HandleFunc("/callback", func(w http.ResponseWriter, r *http.Request) {
		q := r.URL.Query()
		var res result
		switch {
		case subtle.ConstantTimeCompare([]byte(q.Get("state")), []byte(state)) != 1:
			res.err = fmt.Errorf("%w: state mismatch", ErrServer)
		case q.Get("iss") != cred.Issuer:
			// RFC 9207: reject responses from any other authorization server.
			res.err = fmt.Errorf("%w: issuer mismatch", ErrServer)
		case q.Get("error") != "":
			res.err = fmt.Errorf("%w: authorization %s", ErrServer, q.Get("error"))
		case q.Get("code") == "":
			res.err = fmt.Errorf("%w: missing code", ErrServer)
		default:
			res.code = q.Get("code")
		}
		w.Header().Set("content-type", "text/plain; charset=utf-8")
		if res.err != nil {
			w.WriteHeader(http.StatusBadRequest)
			fmt.Fprintln(w, "keyenv-remote: sign-in failed. Return to the terminal.")
		} else {
			fmt.Fprintln(w, "keyenv-remote: signed in. You can close this window.")
		}
		select {
		case done <- res:
		default:
		}
	})
	srv := &http.Server{Handler: mux, ReadHeaderTimeout: 10 * time.Second}
	go srv.Serve(listener)
	defer srv.Close()
	openURL(meta.AuthorizationEndpoint + "?" + query.Encode())
	var res result
	select {
	case res = <-done:
	case <-ctx.Done():
		return nil, ctx.Err()
	}
	if res.err != nil {
		return nil, res.err
	}
	form := url.Values{
		"grant_type":    {"authorization_code"},
		"code":          {res.code},
		"redirect_uri":  {redirect},
		"client_id":     {cred.ClientID},
		"code_verifier": {verifier},
		"resource":      {cred.Resource},
	}
	if err = c.token(ctx, cred, form); err != nil {
		return nil, err
	}
	return cred, c.Save(cred)
}

// Fresh returns credentials with a usable access token, rotating the refresh
// token when needed. The rotated pair is saved before it is used.
func (c *Client) Fresh(ctx context.Context, force bool) (*Credentials, error) {
	unlock, err := lockHome(c.Home)
	if err != nil {
		return nil, err
	}
	defer unlock()
	cred, err := c.Load()
	if err != nil {
		return nil, err
	}
	if !force && cred.ExpiresAt-30 > c.Now().Unix() {
		return cred, nil
	}
	form := url.Values{
		"grant_type":    {"refresh_token"},
		"refresh_token": {cred.RefreshToken},
		"client_id":     {cred.ClientID},
		"resource":      {cred.Resource},
	}
	if err = c.token(ctx, cred, form); err != nil {
		return nil, err
	}
	return cred, c.Save(cred)
}

// ToolError is a tool-level failure reported by the server (never contains values).
type ToolError struct {
	Code    string
	Message string
}

func (e *ToolError) Error() string { return e.Code + ": " + e.Message }

// Call invokes one MCP tool and returns its data object.
func (c *Client) Call(ctx context.Context, tool string, args map[string]any) (map[string]any, error) {
	cred, err := c.Fresh(ctx, false)
	if err != nil {
		return nil, err
	}
	for attempt := 0; ; attempt++ {
		status, envelope, err := c.post(ctx, cred, tool, args)
		if err != nil {
			return nil, err
		}
		if status == http.StatusUnauthorized && attempt == 0 {
			if cred, err = c.Fresh(ctx, true); err != nil {
				return nil, err
			}
			continue
		}
		if status == http.StatusUnauthorized {
			return nil, ErrLoginRequired
		}
		if envelope == nil {
			return nil, fmt.Errorf("%w: MCP returned %d", ErrServer, status)
		}
		if ok, _ := envelope["ok"].(bool); !ok {
			e, _ := envelope["error"].(map[string]any)
			code, _ := e["code"].(string)
			message, _ := e["message"].(string)
			return nil, &ToolError{Code: code, Message: message}
		}
		data, _ := envelope["data"].(map[string]any)
		return data, nil
	}
}

func (c *Client) post(ctx context.Context, cred *Credentials, tool string, args map[string]any) (int, map[string]any, error) {
	body, err := json.Marshal(map[string]any{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": map[string]any{"name": tool, "arguments": args}})
	if err != nil {
		return 0, nil, err
	}
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, cred.Resource, bytes.NewReader(body))
	if err != nil {
		return 0, nil, err
	}
	req.Header.Set("content-type", "application/json")
	req.Header.Set("accept", "application/json, text/event-stream")
	req.Header.Set("mcp-protocol-version", ProtocolVersion)
	req.Header.Set("authorization", "Bearer "+cred.AccessToken)
	resp, err := c.HTTP.Do(req)
	if err != nil {
		return 0, nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		io.Copy(io.Discard, io.LimitReader(resp.Body, maxResponse))
		return resp.StatusCode, nil, nil
	}
	var rpc struct {
		Result struct {
			Structured map[string]any `json:"structuredContent"`
		} `json:"result"`
		Error *struct {
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, 8*maxResponse)).Decode(&rpc); err != nil {
		return resp.StatusCode, nil, fmt.Errorf("%w: invalid MCP response", ErrServer)
	}
	if rpc.Error != nil {
		return resp.StatusCode, nil, fmt.Errorf("%w: %s", ErrServer, rpc.Error.Message)
	}
	return resp.StatusCode, rpc.Result.Structured, nil
}
