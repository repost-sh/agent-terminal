#!/usr/bin/env bash

set -euo pipefail

if [[ -t 1 ]] && command -v tput >/dev/null 2>&1 && [[ "$(tput colors 2>/dev/null || echo 0)" -ge 8 ]]; then
  BOLD=$(tput bold); DIM=$(tput dim); RESET=$(tput sgr0)
  BLUE=$(tput setaf 4); GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3)
else
  BOLD=""; DIM=""; RESET=""; BLUE=""; GREEN=""; YELLOW=""
fi

TOTAL_STAGES=0

_STAGE_INDEX=0
ENV_FILE="${ENV_FILE:-.env}"
WRITTEN_ENV=()
WRITTEN_SECRET=()
SKIPPED=()

_clear() {
  [[ -t 1 ]] || return 0
  if command -v tput >/dev/null 2>&1 && tput clear 2>/dev/null; then return 0; fi
  printf '\033[2J\033[3J\033[H'
}

banner() {
  _clear
  printf '\n%s%s  %s%s\n' "$BOLD" "$BLUE" "$1" "$RESET"
  printf '%s  %s stages%s\n\n' "$DIM" "$TOTAL_STAGES" "$RESET"
  printf '%s  You drive the browser; this wizard tells you exactly what to do and\n' "$DIM"
  printf '  captures the values you copy back. Stop any time with Ctrl-C and re-run\n'
  printf '  later, since it remembers values already saved.%s\n' "$RESET"
  pause "Ready to start?"
}

stage() {
  _clear
  _STAGE_INDEX=$((_STAGE_INDEX + 1))
  printf '\n%s%s▸ Stage %s/%s · %s%s\n' \
    "$BOLD" "$BLUE" "$_STAGE_INDEX" "$TOTAL_STAGES" "$1" "$RESET"
}

say()  { printf '  %s\n' "$1"; }
step() { printf '  %s•%s %s\n' "$BLUE" "$RESET" "$1"; }
note() { printf '  %s%s%s\n' "$DIM" "$1" "$RESET"; }
warn() { printf '  %s⚠ %s%s\n' "$YELLOW" "$1" "$RESET"; }

open_url() {
  local url="$1"
  printf '  %s↗ opening%s %s\n' "$GREEN" "$RESET" "$url"
  { if   command -v wslview     >/dev/null 2>&1; then wslview "$url"
    elif command -v explorer.exe >/dev/null 2>&1; then explorer.exe "$url"
    elif command -v xdg-open    >/dev/null 2>&1; then xdg-open "$url"
    elif command -v open        >/dev/null 2>&1; then open "$url"
    else false; fi
  } >/dev/null 2>&1 || warn "couldn't open a browser, so visit it manually: $url"
}

pause() {
  printf '  %s%s%s ' "$DIM" "${1:-Press Enter to continue}" "$RESET"
  read -r _ || true
}

confirm() {
  local reply=""
  printf '  %s? %s [y/N] ' "$YELLOW" "$1"
  read -r reply || true
  [[ "$reply" =~ ^[Yy] ]]
}

_existing() {
  [[ -f "$ENV_FILE" ]] || return 1
  local line value sq="'\\''"; line=$(grep -E "^${1}=" "$ENV_FILE" | tail -n1) || return 1
  value="${line#*=}"
  if [[ "$value" == \'*\' ]]; then value="${value:1:${#value}-2}"; value="${value//"$sq"/\'}"
  elif [[ "$value" == \"*\" ]]; then
    value="${value:1:${#value}-2}"
    [[ "$value" == *[\\\$\"]* ]] && return 1
  fi
  printf '%s' "$value"
}

ask() {
  local key="$1" prompt="$2" current input s=$'\001' e=$'\002' p rc=0
  current=$(_existing "$key" || true)
  p="  $s$BOLD$e$prompt$s$RESET$e "
  [[ -n "$current" ]] && p+="$s$DIM${e}[Enter keeps current]$s$RESET$e "
  read -e -r -p "$p" input || rc=1
  if (( rc )) && [[ -z "$input" ]]; then return 1; fi
  [[ -z "$input" && -n "$current" ]] && input="$current"
  printf -v "$key" '%s' "$input"
}

ask_secret() {
  local key="$1" prompt="$2" current input rc=0
  current=$(_existing "$key" || true)
  if [[ -n "$current" ]]; then
    printf '  %s%s%s %s[Enter keeps current]%s ' "$BOLD" "$prompt" "$RESET" "$DIM" "$RESET"
  else
    printf '  %s%s%s ' "$BOLD" "$prompt" "$RESET"
  fi
  read -rs input || rc=1
  printf '\n'
  if (( rc )) && [[ -z "$input" ]]; then return 1; fi
  [[ -z "$input" && -n "$current" ]] && input="$current"
  printf -v "$key" '%s' "$input"
}

write_env() {
  local key="$1" value="$2" tmp sq="'\\''"
  [[ -e "$ENV_FILE" ]] || (umask 077 && : > "$ENV_FILE")
  tmp=$(mktemp)
  grep -vE "^${key}=" "$ENV_FILE" > "$tmp" || true
  printf "%s='%s'\n" "$key" "${value//\'/$sq}" >> "$tmp"
  cat "$tmp" > "$ENV_FILE"
  rm -f "$tmp"
  printf -v "$key" '%s' "$value"
  WRITTEN_ENV+=("$key")
  printf '  %s✓ wrote%s %s → %s\n' "$GREEN" "$RESET" "$key" "$ENV_FILE"
}

set_secret() {
  local name="$1" value="$2"
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    if printf '%s' "$value" | gh secret set "$name" >/dev/null 2>&1; then
      WRITTEN_SECRET+=("$name")
      printf '  %s✓ set%s GitHub secret %s\n' "$GREEN" "$RESET" "$name"
      return
    fi
  fi
  SKIPPED+=("GitHub secret $name (set it manually: gh secret set $name)")
  warn "skipped GitHub secret $name: gh not ready; set it later"
}

finish() {
  _clear
  printf '\n%s%s  ✓ Setup complete%s\n' "$BOLD" "$GREEN" "$RESET"
  (( ${#WRITTEN_ENV[@]} ))    && note "wrote ${#WRITTEN_ENV[@]} value(s) to $ENV_FILE: ${WRITTEN_ENV[*]}"
  (( ${#WRITTEN_SECRET[@]} )) && note "set ${#WRITTEN_SECRET[@]} GitHub secret(s): ${WRITTEN_SECRET[*]}"
  if (( ${#SKIPPED[@]} )); then
    printf '\n'; warn "still to do by hand:"
    for s in "${SKIPPED[@]}"; do note "  - $s"; done
  fi
  printf '\n'
}

TAP_REPO=repost-sh/homebrew-tap
SOURCE_REPO=repost-sh/agent-terminal
PACKAGES=(
  @repost/agent-terminal
  @repost/agent-terminal-darwin-arm64
  @repost/agent-terminal-darwin-x64
  @repost/agent-terminal-linux-arm64
  @repost/agent-terminal-linux-x64
  @repost/agent-terminal-linux-arm64-musl
  @repost/agent-terminal-linux-x64-musl
)
PLACEHOLDER_VERSION=0.0.0-bootstrap
NPM_REGISTRY=https://registry.npmjs.org/
OIDC_PUBLISHER="<npm-oidc-no-reply@github.com>"
TRUST_WINDOW_HOURS=48
TAP_TOKEN_PAGE="https://github.com/settings/personal-access-tokens/new?name=agent-terminal+homebrew+tap&description=Lets+agent-terminal+releases+push+the+Homebrew+formula&target_name=repost-sh&expires_in=366&contents=write"
USAGE="usage: scripts/one-time-setup.sh [--check]"

STATUS=()
TRUST=()
AGE_HOURS=()
DONE_NOW=()

die() { printf '  %s✗ %s%s\n' "$YELLOW" "$1" "$RESET" >&2; exit 1; }
ok() { printf '  %s✓%s %s\n' "$GREEN" "$RESET" "$1"; }

stop() {
  printf '\n'
  warn "Stopped: $1"
  note "Run scripts/one-time-setup.sh again when you're ready; it skips what's already done."
  exit 1
}

remembered() { _existing "$1" || true; }

trusted_publisher_key() {
  printf 'NPM_TRUSTED_PUBLISHER_%s' "$(printf '%s' "${1#@repost/}" | tr '[:lower:]-' '[:upper:]_')"
}

tokens_page() { printf 'https://www.npmjs.com/settings/%s/tokens' "$NPM_USERNAME"; }

package_status() {
  local out
  if out=$(npm view "$1" _npmUser 2>&1); then
    if [[ "$out" == *"$OIDC_PUBLISHER"* ]]; then printf 'released'; else printf 'placeholder'; fi
  elif [[ "$out" == *E404* ]]; then
    printf 'missing'
  else
    printf '%s\n' "$out" >&2
    return 1
  fi
}

gather() {
  local out names i added created deleted now
  if out=$(gh repo view "$TAP_REPO" --json name 2>&1); then
    TAP_EXISTS=yes
  elif [[ "$out" == *"Could not resolve to a Repository"* ]]; then
    TAP_EXISTS=no
  else
    printf '%s\n' "$out" >&2
    die "gh couldn't look up $TAP_REPO."
  fi

  names=$(gh secret list -R "$SOURCE_REPO" --json name --jq '.[].name') \
    || die "gh couldn't list the secrets of $SOURCE_REPO; that needs admin access to it."
  SECRET_SET=no
  if grep -qx HOMEBREW_TAP_TOKEN <<<"$names"; then SECRET_SET=yes; fi

  now=$(date +%s)
  MISSING=0
  TRUST_TODO=0
  for i in "${!PACKAGES[@]}"; do
    STATUS[i]=$(package_status "${PACKAGES[i]}") || die "npm couldn't look up ${PACKAGES[i]}."
    added=$(remembered "$(trusted_publisher_key "${PACKAGES[i]}")")
    AGE_HOURS[i]=""
    if [[ ${STATUS[i]} == released ]]; then
      TRUST[i]=released
    elif [[ ${STATUS[i]} == missing ]]; then
      TRUST[i]=none
      MISSING=$((MISSING + 1))
    elif [[ -z "$added" ]]; then
      TRUST[i]=none
    else
      AGE_HOURS[i]=$(( (now - added) / 3600 ))
      if (( now - added < TRUST_WINDOW_HOURS * 3600 )); then TRUST[i]=waiting; else TRUST[i]=expired; fi
    fi
    if [[ ${TRUST[i]} == none || ${TRUST[i]} == expired ]]; then TRUST_TODO=$((TRUST_TODO + 1)); fi
  done

  created=$(remembered NPM_BOOTSTRAP_TOKEN_CREATED)
  deleted=$(remembered NPM_BOOTSTRAP_TOKEN_DELETED)
  TOKEN_LEFT=no
  if [[ -n "$created" ]] && { [[ -z "$deleted" ]] || (( deleted < created )); }; then TOKEN_LEFT=yes; fi
}

plan() {
  TOTAL_STAGES=$TRUST_TODO
  if [[ $TAP_EXISTS == no ]]; then TOTAL_STAGES=$((TOTAL_STAGES + 1)); fi
  if [[ $SECRET_SET == no ]]; then TOTAL_STAGES=$((TOTAL_STAGES + 1)); fi
  if (( MISSING )); then TOTAL_STAGES=$((TOTAL_STAGES + 2)); fi
  if (( MISSING )) || [[ $TOKEN_LEFT == yes ]]; then TOTAL_STAGES=$((TOTAL_STAGES + 1)); fi
}

status_line() {
  case $1 in
    ready) printf '  %s✓%s %s\n' "$GREEN" "$RESET" "$2" ;;
    waiting) printf '  %s…%s %s\n' "$BLUE" "$RESET" "$2" ;;
    todo) printf '  %s✗%s %s\n' "$YELLOW" "$RESET" "$2" ;;
  esac
}

report() {
  local i name
  if [[ $TAP_EXISTS == yes ]]; then
    status_line ready "$TAP_REPO exists"
  else
    status_line todo "$TAP_REPO doesn't exist yet"
  fi
  if [[ $SECRET_SET == yes ]]; then
    status_line ready "HOMEBREW_TAP_TOKEN is set on $SOURCE_REPO"
  else
    status_line todo "HOMEBREW_TAP_TOKEN isn't set on $SOURCE_REPO"
  fi
  for i in "${!PACKAGES[@]}"; do
    name=${PACKAGES[i]}
    case "${STATUS[i]}:${TRUST[i]}" in
      released:*) status_line ready "$name: a release published it through its trusted publisher" ;;
      missing:*) status_line todo "$name: not on npm yet, so it can't have a trusted publisher" ;;
      *:waiting) status_line waiting "$name: trusted publisher added ${AGE_HOURS[i]}h ago; npm drops it unless a release publishes within ${TRUST_WINDOW_HOURS}h of adding it" ;;
      *:expired) status_line todo "$name: trusted publisher added ${AGE_HOURS[i]}h ago has expired, since no release published within ${TRUST_WINDOW_HOURS}h" ;;
      *) status_line todo "$name: no trusted publisher yet" ;;
    esac
  done
  if [[ $TOKEN_LEFT == yes ]]; then
    status_line todo "the npm bootstrap token 'agent-terminal bootstrap' hasn't been deleted"
  fi
}

ask_npm_username() {
  NPM_USERNAME=""
  while [[ -z "$NPM_USERNAME" ]]; do
    ask NPM_USERNAME "Your npm username:" || stop "no npm username was given."
  done
  write_env NPM_USERNAME "$NPM_USERNAME"
}

write_placeholder() {
  local dir="$WORK/placeholders/${1#@repost/}"
  mkdir -p "$dir"
  cat > "$dir/package.json" <<EOF
{
  "name": "$1",
  "version": "$PLACEHOLDER_VERSION",
  "description": "Placeholder published once so npm trusted publishing could be set up for agent-terminal; install 0.1.0 or later",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/repost-sh/agent-terminal.git"
  },
  "publishConfig": {
    "access": "public"
  }
}
EOF
  printf '%s' "$dir"
}

with_bootstrap_token() {
  NPM_BOOTSTRAP_TOKEN="$NPM_BOOTSTRAP_TOKEN" npm_config_userconfig="$PUBLISH_NPMRC" "$@"
}

stage_tap_repository() {
  stage "Homebrew: the tap repository"
  say "Each release pushes its Homebrew formula to $TAP_REPO; that is what makes"
  say "\`brew install repost-sh/tap/agent-terminal\` work. The repository doesn't exist yet."
  step "This creates it public, with a README so it starts with a main branch for the release to push to."
  if ! confirm "Create the public repository $TAP_REPO now?"; then stop "$TAP_REPO wasn't created."; fi
  gh repo create "$TAP_REPO" --public --add-readme --description "Homebrew formulae for repost-sh tools" >/dev/null
  TAP_EXISTS=yes
  DONE_NOW+=("created https://github.com/$TAP_REPO")
  ok "created https://github.com/$TAP_REPO"
}

stage_tap_token() {
  stage "Homebrew: the token the release pushes with"
  say "The release's Homebrew job pushes the formula with this token, kept as the"
  say "HOMEBREW_TAP_TOKEN secret on $SOURCE_REPO. It needs write access to the tap's contents, nothing more."
  open_url "$TAP_TOKEN_PAGE"
  step "Name, description, resource owner (repost-sh), expiration (366 days) and the Contents permission are filled in."
  step "Repository access: Only select repositories, then pick $TAP_REPO."
  step "Permissions: check that Contents says Read and write (Metadata: Read-only comes with it)."
  step "Click Generate token and copy the token."
  note "If repost-sh isn't offered as the resource owner, allow fine-grained tokens in the organization's settings, Personal access tokens."
  note "When it expires the Homebrew publish fails: run gh secret delete HOMEBREW_TAP_TOKEN -R $SOURCE_REPO, then this script again."
  while :; do
    ask_secret TAP_TOKEN "Paste the token:" || stop "no token was pasted."
    if [[ -n "$TAP_TOKEN" ]] && GH_TOKEN="$TAP_TOKEN" gh api "repos/$TAP_REPO" --silent >/dev/null 2>&1; then break; fi
    warn "That token can't read $TAP_REPO. Copy the token you just generated, with $TAP_REPO selected, and paste it again."
  done
  set_secret HOMEBREW_TAP_TOKEN "$TAP_TOKEN"
  unset TAP_TOKEN
}

stage_npm_token() {
  stage "npm: a short-lived token for the first publish"
  say "npm adds a trusted publisher only to a package that already exists: $MISSING of the ${#PACKAGES[@]} packages aren't on npm yet."
  say "Each of them gets a placeholder version, $PLACEHOLDER_VERSION, published once with a token you delete"
  say "right after. The release then publishes every real version through trusted publishing, without a token."
  ask_npm_username
  write_env NPM_BOOTSTRAP_TOKEN_CREATED "$(date +%s)"
  open_url "$(tokens_page)"
  step "Generate New Token, a granular access token. Token name: agent-terminal bootstrap."
  step "Bypass two-factor authentication: check it, so the publishes don't stop for a code."
  step "Packages and scopes: Read and write (publish and stage); Only select packages and scopes, the @repost scope."
  note "If the @repost scope isn't offered, or npm later refuses to publish with it, choose All Packages instead."
  step "Organizations: No access."
  step "Expiration: Custom, tomorrow's date, the shortest npm allows."
  step "Click Generate Token and copy the token."
  while :; do
    ask_secret NPM_BOOTSTRAP_TOKEN "Paste the token:" \
      || stop "no token was pasted. Delete the token if you created one: $(tokens_page)"
    if [[ -n "$NPM_BOOTSTRAP_TOKEN" ]] && with_bootstrap_token npm whoami >/dev/null 2>&1; then break; fi
    warn "npm doesn't accept that token. Copy it again from the page and paste it."
  done
  ok "npm accepts the token"
}

stage_publish_placeholders() {
  local i dir
  stage "npm: publish the placeholders"
  say "These packages get version $PLACEHOLDER_VERSION under the dist-tag bootstrap:"
  for i in "${!PACKAGES[@]}"; do
    if [[ ${STATUS[i]} == missing ]]; then step "${PACKAGES[i]}"; fi
  done
  say "npm never frees a version number, so they stay listed; the first release publishes 0.1.0 as latest."
  if ! confirm "Publish them now?"; then
    stop "nothing was published. Delete the token agent-terminal bootstrap: $(tokens_page)"
  fi
  for i in "${!PACKAGES[@]}"; do
    if [[ ${STATUS[i]} != missing ]]; then continue; fi
    dir=$(write_placeholder "${PACKAGES[i]}")
    with_bootstrap_token npm publish "$dir" --access public --tag bootstrap --loglevel error >/dev/null \
      || stop "npm didn't publish ${PACKAGES[i]}; fix what it says above, then run this again."
    STATUS[i]=placeholder
    DONE_NOW+=("published ${PACKAGES[i]}@$PLACEHOLDER_VERSION")
    ok "published ${PACKAGES[i]}@$PLACEHOLDER_VERSION"
  done
  rm -rf "$WORK/placeholders"
}

stage_delete_npm_token() {
  stage "npm: delete the bootstrap token"
  NPM_USERNAME=$(remembered NPM_USERNAME)
  if [[ -z "$NPM_USERNAME" ]]; then ask_npm_username; fi
  say "The placeholders are on npm, so the token agent-terminal bootstrap has done its job."
  open_url "$(tokens_page)"
  step "Find agent-terminal bootstrap in the token list, click the × next to it and confirm."
  note "npm says a deleted token can take up to an hour to stop working."
  if confirm "Deleted it?"; then
    write_env NPM_BOOTSTRAP_TOKEN_DELETED "$(date +%s)"
    DONE_NOW+=("deleted the npm token agent-terminal bootstrap")
  else
    SKIPPED+=("delete the npm token agent-terminal bootstrap: $(tokens_page)")
  fi
  unset NPM_BOOTSTRAP_TOKEN
}

stage_trusted_publisher() {
  local i=$1 name=${PACKAGES[$1]} page="https://www.npmjs.com/package/${PACKAGES[$1]}/access"
  stage "npm: trusted publisher for $name"
  if [[ ${TRUST[i]} == expired ]]; then
    warn "The trusted publisher added ${AGE_HOURS[i]}h ago has expired: no release published within ${TRUST_WINDOW_HOURS}h."
    step "Delete the expired trusted publisher on the page first."
  fi
  say "This lets the release publish $name from GitHub Actions, with provenance and no token."
  open_url "$page"
  step "In the Trusted Publisher section, choose GitHub Actions."
  step "Organization or user: repost-sh"
  step "Repository: agent-terminal"
  step "Workflow filename: release.yml (the workflow the tag starts; it calls publish-npm.yml)"
  step "Environment name: leave it empty."
  step "Save it."
  note "npm drops it unless a release publishes within ${TRUST_WINDOW_HOURS}h, so tag the release within two days."
  if confirm "Saved?"; then
    write_env "$(trusted_publisher_key "$name")" "$(date +%s)"
    DONE_NOW+=("trusted publisher for $name")
  else
    SKIPPED+=("the trusted publisher for $name: $page")
  fi
}

run_wizard() {

case "${1:-}" in
  ""|--check) ;;
  *) die "$USAGE" ;;
esac

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK=$ROOT/.superpowers/one-time-setup
ENV_FILE=$WORK/state.env
READ_NPMRC=$WORK/npmrc
PUBLISH_NPMRC=$WORK/npmrc-publish
mkdir -p "$WORK/tmp"
export TMPDIR=$WORK/tmp GH_REPO=$SOURCE_REPO
export npm_config_userconfig=$READ_NPMRC npm_config_cache=$WORK/npm-cache npm_config_update_notifier=false
printf 'registry=%s\n@repost:registry=%s\n' "$NPM_REGISTRY" "$NPM_REGISTRY" > "$READ_NPMRC"
{ cat "$READ_NPMRC"; printf '%s\n' "//registry.npmjs.org/:_authToken=\${NPM_BOOTSTRAP_TOKEN}"; } > "$PUBLISH_NPMRC"

command -v gh >/dev/null 2>&1 || die "Install the GitHub CLI (https://cli.github.com), then run gh auth login."
command -v npm >/dev/null 2>&1 || die "Install Node.js with npm first."
gh auth status >/dev/null 2>&1 || die "Log in to GitHub first: gh auth login"

gather
plan

if [[ "${1:-}" == --check ]]; then
  report
  if (( TOTAL_STAGES )); then exit 1; fi
  exit 0
fi

if (( TOTAL_STAGES == 0 )); then
  report
  printf '\n'
  ok "Nothing left to do."
  exit 0
fi

banner "agent-terminal one-time account setup"

if [[ $TAP_EXISTS == no ]]; then stage_tap_repository; fi
if [[ $SECRET_SET == no ]]; then stage_tap_token; fi
if (( MISSING )); then
  stage_npm_token
  stage_publish_placeholders
fi
if (( MISSING )) || [[ $TOKEN_LEFT == yes ]]; then stage_delete_npm_token; fi
for i in "${!PACKAGES[@]}"; do
  if [[ ${TRUST[i]} == none || ${TRUST[i]} == expired ]]; then stage_trusted_publisher "$i"; fi
done

finish
if (( ${#DONE_NOW[@]} )); then
  for item in "${DONE_NOW[@]}"; do ok "$item"; done
  printf '\n'
fi
note "npm drops a new trusted publisher that no release publishes with within ${TRUST_WINDOW_HOURS}h: tag the release by then."
note "See where things stand any time: scripts/one-time-setup.sh --check"
}

run_wizard "$@"; exit
