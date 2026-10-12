package drive

import (
	"context"
	"errors"
	"io"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"golang.org/x/oauth2"
)

type TokenCache interface {
	Load() (*oauth2.Token, error)
	Save(*oauth2.Token) error
}

// ValidCachedToken rejects incomplete/non-Bearer credentials and unbounded input.
// Expired access tokens are valid cache entries: they must use the refresh token.
func ValidCachedToken(t *oauth2.Token) bool {
	if t == nil || t.AccessToken == "" || t.RefreshToken == "" || t.Expiry.IsZero() || t.Type() != "Bearer" {
		return false
	}
	for _, v := range []string{t.AccessToken, t.RefreshToken} {
		if len(v) > 8192 || strings.ContainsAny(v, "\r\n\x00\t ") {
			return false
		}
	}
	return true
}
func validateGrantedScope(t *oauth2.Token) error {
	if scope, ok := t.Extra("scope").(string); ok && strings.TrimSpace(scope) != DriveFileScope {
		return vault.ErrAuthRequired
	}
	return nil
}

// AuthorizeCached uses only the existing Desktop client and drive.file. A missing
// cache requires one personal consent; invalid/expired grants fail explicitly.
// Browser authorization is never silently restarted after a refresh failure.
func AuthorizeCached(ctx context.Context, clientPath, account string, instructions io.Writer, cache TokenCache) (*http.Client, error) {
	return AuthorizeCachedWithConsent(ctx, clientPath, account, instructions, cache, false)
}

// Force consent is explicit user recovery; it never deletes or replaces a cache
// before a successful grant with the correct account and scope.
func AuthorizeCachedWithConsent(ctx context.Context, clientPath, account string, instructions io.Writer, cache TokenCache, force bool) (*http.Client, error) {
	return authorizeCached(ctx, clientPath, account, instructions, cache, force, false)
}

// RefreshCached explicitly exercises renewal without browser consent or writing
// an artificially expired token to disk. Failure preserves the previous cache.
func RefreshCached(ctx context.Context, clientPath, account string, cache TokenCache) (*http.Client, error) {
	return authorizeCached(ctx, clientPath, account, io.Discard, cache, false, true)
}
func authorizeCached(ctx context.Context, clientPath, account string, instructions io.Writer, cache TokenCache, force, refresh bool) (*http.Client, error) {
	ctx = secureOAuthContext(ctx)
	cfg, err := readOAuthConfig(clientPath)
	if err != nil {
		return nil, err
	}
	token, err := cache.Load()
	if err != nil {
		return nil, err
	}
	if refresh && token == nil {
		return nil, vault.ErrAuthRequired
	}
	if token == nil || force {
		_, token, err = AuthorizeForSetup(ctx, clientPath, account, instructions)
		if err != nil {
			return nil, err
		}
		if err = cache.Save(token); err != nil {
			return nil, err
		}
	}
	initial := token
	if refresh {
		expired := *token
		expired.Expiry = time.Now().Add(-time.Minute)
		initial = &expired
	}
	source := newSavingSource(cfg.TokenSource(ctx, initial), cache, initial)
	if _, err = source.Token(); err != nil {
		return nil, err
	}
	return oauthClient(ctx, source), nil
}

type savingSource struct {
	mu     sync.Mutex
	source oauth2.TokenSource
	cache  TokenCache
	last   oauth2.Token
}

func newSavingSource(source oauth2.TokenSource, cache TokenCache, initial *oauth2.Token) *savingSource {
	return &savingSource{source: source, cache: cache, last: *initial}
}
func (s *savingSource) Token() (*oauth2.Token, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	token, err := s.source.Token()
	if err != nil {
		var response *oauth2.RetrieveError
		if errors.As(err, &response) && (response.ErrorCode == "invalid_grant" || response.ErrorCode == "invalid_client" || response.ErrorCode == "unauthorized_client") {
			return nil, vault.ErrAuthRequired
		}
		return nil, vault.ErrUnavailable
	}
	if !ValidCachedToken(token) || validateGrantedScope(token) != nil {
		return nil, vault.ErrAuthRequired
	}
	if token.AccessToken != s.last.AccessToken || token.RefreshToken != s.last.RefreshToken || token.TokenType != s.last.TokenType || !token.Expiry.Equal(s.last.Expiry) {
		if err = s.cache.Save(token); err != nil {
			return nil, err
		}
		s.last = *token
	}
	return token, nil
}

// New-device setup cannot encrypt its token until it obtains and unlocks the
// remote vault header. It keeps this grant in memory until that point.
func AuthorizeForSetup(ctx context.Context, clientPath, account string, instructions io.Writer) (*http.Client, *oauth2.Token, error) {
	ctx = secureOAuthContext(ctx)
	cfg, err := readOAuthConfig(clientPath)
	if err != nil {
		return nil, nil, err
	}
	token, err := browserAuthorize(ctx, cfg, account, instructions, true)
	if err != nil {
		return nil, nil, err
	}
	if !ValidCachedToken(token) || validateGrantedScope(token) != nil {
		return nil, nil, vault.ErrAuthRequired
	}
	client := oauthClient(ctx, cfg.TokenSource(ctx, token))
	actual, err := (&Store{Client: client}).Account(ctx)
	if err != nil {
		return nil, nil, err
	}
	if actual != account {
		return nil, nil, vault.ErrAuthRequired
	}
	return client, token, nil
}
