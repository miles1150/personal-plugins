---
name: keyenv-remote
description: Use the user's personal secrets from the remote keyenv store (works.miles1150.top) through its MCP tools list_entries, fetch_entry, create_entry, update_entry and disable_entry. Use when a task needs an API key, token or password the user keeps in keyenv, or when the user asks to save or change one. Works in cloud sessions without any CLI.
---

# keyenv-remote

The user's secrets live in a remote store reached only through the MCP server `miles-works` (`https://works.miles1150.top/mcp`). Its tools are the whole interface. Do not download, install or run any keyenv binary in this session, and do not ask the user for a client ID, token or API key to connect.

These rules are discipline, not a sandbox. A value you fetch passes through this conversation's tool results, and a value the user types into chat is already in the conversation. The rules keep values out of replies, files under version control, logs and command lines; they cannot make the model unable to see them.

## Connect

If the tools are missing or return an authentication error, ask the user to connect the `miles-works` MCP server in their host (install the plugin or add the remote MCP server, click Connect, sign in with Google, review message and key permissions; key read, change and create access are disclosed separately). Whether a new cloud session reuses authorization depends on the host and has not yet been verified end to end. Never ask the user to paste tokens or client IDs into chat.

Use `works_identity` to verify the selected connection and its key capability before calling key tools. Preserve old connections; the new service does not import the old vault automatically. Let the user complete login and consent.

## Find the entry

1. Call `list_entries`. It returns names, purposes, field names, and whether this connection may read or change each entry. It never returns values.
2. Pick the entry by name and purpose. If more than one could fit, or none clearly does, ask the user. Do not guess.
3. Do not use an entry that is `disabled` or past `expires`. Tell the user instead.
4. An entry missing from the list may exist but not be shared with this connection. Ask the user to reconnect and allow it; do not look for workarounds.

## Use a value

1. Create a private directory outside the repository and the workspace's version control, and restrict it before writing anything:

   ```bash
   dir="${TMPDIR:-/tmp}/keyenv-remote"; mkdir -p "$dir"; chmod 700 "$dir"
   ```

   If the file must live inside a repository, first confirm `git check-ignore -q <path>` succeeds; if it does not, use the directory above instead.
2. Call `fetch_entry` with the entry name, and with `fields` when only some fields are needed.
3. In one step, write the returned `fields` object as JSON to `$dir/<entry name with / replaced by _>.json` using a file-writing tool, then `chmod 600` it. Do not pass values through `echo`, here-strings or command arguments.
4. Later commands read the file themselves, for example `export GH_TOKEN="$(jq -r .GH_TOKEN "$f")"` or a program that loads JSON. Never put a value in a command line, URL, commit, log or environment dump. Never `cat` or print the file.
5. In replies, mention only the file path and field names. Never quote, paraphrase, abbreviate or partially mask a value.
6. Delete the file when the task no longer needs it.

## Save or change a value

- New entry: `create_entry` with `name` (lowercase, `a-z0-9/_-`, starts with a letter), optional `purpose` and `expires` (`YYYY-MM-DD`), and `fields` (`{"FIELD_NAME": "value"}`; field names are environment variable names).
- Existing entry: `update_entry` with `set_fields`, `remove_fields`, `purpose` or `expires` (`null` clears it). `disable_entry` disables or re-enables an entry.
- The results contain the entry name and field names only. Confirm the save using those. Do not repeat, summarize or mask the value the user gave you, and do not write it to any file.
- A value the user typed into chat has already passed through the conversation. For high-value secrets, suggest the user run `keyenv-remote set NAME` on their own computer instead; that path never shows the value to an agent.
- `CREATE_NOT_GRANTED`, `WRITE_NOT_GRANTED` or `READ_NOT_GRANTED` means this connection was not allowed that action. Tell the user which permission is missing; they grant it by reconnecting. `ENTRY_CHANGED` means retry after listing again.

## Never

- Never fetch an entry the task does not need, or more fields than it needs.
- Never store values in the repository, in memory or notes files, in issue or PR text, or in this skill's outputs.
- Never send a value to any destination other than the program that needs it.
