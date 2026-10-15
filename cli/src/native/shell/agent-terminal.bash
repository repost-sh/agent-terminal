if [ -f ~/.bashrc ]; then
  . ~/.bashrc
fi

__agent_terminal_running=
__agent_terminal_armed=
__agent_terminal_code=0

__agent_terminal_encode() {
  local LC_ALL=C text=$1 out= byte code number i
  for (( i = 0; i < ${#text}; i++ )); do
    byte=${text:i:1}
    case $byte in
      [A-Za-z0-9/._~-]) out+=$byte ;;
      *)
        printf -v number '%d' "'$byte"
        printf -v code '%%%02X' $(( number & 255 ))
        out+=$code
        ;;
    esac
  done
  __agent_terminal_encoded=$out
}

__agent_terminal_precmd() {
  __agent_terminal_code=$?
  if [ -n "$__agent_terminal_running" ]; then
    __agent_terminal_running=
    printf '\e]133;D;%s\a' "$__agent_terminal_code"
  fi
  __agent_terminal_encode "$PWD"
  printf '\e]133;A\a\e]7;file://%s\a\e]2;%s\a' "$__agent_terminal_encoded" "${PWD//[[:cntrl:]]/ }"
  return $__agent_terminal_code
}

__agent_terminal_entry='^ *([0-9]+)[* ] (.*)$'

__agent_terminal_record_every_line() {
  [ -n "$__agent_terminal_recording" ] && __agent_terminal_keep_users_history
  __agent_terminal_histcontrol=${HISTCONTROL-}
  __agent_terminal_histcontrol_set=${HISTCONTROL+set}
  __agent_terminal_histignore=${HISTIGNORE-}
  __agent_terminal_histignore_set=${HISTIGNORE+set}
  local IFS=: value kept=
  local -a values
  read -ra values <<<"$__agent_terminal_histcontrol"
  for value in "${values[@]}"; do
    case $value in
      ignorespace) ;;
      ignoreboth) kept+=${kept:+:}ignoredups ;;
      *) kept+=${kept:+:}$value ;;
    esac
  done
  HISTCONTROL=$kept
  HISTIGNORE=
  __agent_terminal_last_entry=
  [[ $(HISTTIMEFORMAT= builtin history 1) =~ $__agent_terminal_entry ]] && __agent_terminal_last_entry=${BASH_REMATCH[1]}
  __agent_terminal_recording=1
}

__agent_terminal_keep_users_history() {
  if [ -n "$__agent_terminal_histcontrol_set" ]; then HISTCONTROL=$__agent_terminal_histcontrol; else unset HISTCONTROL; fi
  if [ -n "$__agent_terminal_histignore_set" ]; then HISTIGNORE=$__agent_terminal_histignore; else unset HISTIGNORE; fi
  __agent_terminal_recording=
}

__agent_terminal_left_out() {
  local line=$1 pattern previous
  case :$__agent_terminal_histcontrol: in
    *:ignorespace:* | *:ignoreboth:*) [[ $line == ' '* ]] && return 0 ;;
  esac
  local IFS=:
  local -a patterns
  read -ra patterns <<<"$__agent_terminal_histignore"
  for pattern in "${patterns[@]}"; do
    if [ "$pattern" = '&' ]; then
      previous=$(HISTTIMEFORMAT= builtin history 2)
      [[ ${previous%%$'\n'*} =~ $__agent_terminal_entry ]] && [ "$line" = "${BASH_REMATCH[2]}" ] && return 0
    elif [[ $line == $pattern ]]; then
      return 0
    fi
  done
  return 1
}

__agent_terminal_ready() {
  __agent_terminal_armed=1
  case $PS1 in
    *$'\e]133;B\a'*) ;;
    *) PS1=$PS1$'\[\e]133;B\a\]' ;;
  esac
  __agent_terminal_record_every_line
  return $__agent_terminal_code
}

__agent_terminal_preexec() {
  [ -n "$__agent_terminal_armed" ] || return 0
  __agent_terminal_armed=
  [ "$BASH_COMMAND" = __agent_terminal_precmd ] && return 0
  [ -n "${COMP_LINE-}" ] && return 0
  __agent_terminal_keep_users_history
  __agent_terminal_running=1
  local line
  if [[ :$SHELLOPTS: == *:history:* && $(HISTTIMEFORMAT= builtin history 1) =~ $__agent_terminal_entry ]]; then
    line=${BASH_REMATCH[2]}
    if [ "${BASH_REMATCH[1]}" != "$__agent_terminal_last_entry" ] && __agent_terminal_left_out "$line"; then
      builtin history -d "${BASH_REMATCH[1]}"
    fi
  else
    line=$BASH_COMMAND
  fi
  __agent_terminal_encode "$line"
  printf '\e]133;C;cmdline_url=%s\a\e]2;%s\a' "$__agent_terminal_encoded" "${line//[[:cntrl:]]/ }"
}

if [ -n "$PROMPT_COMMAND" ]; then
  PROMPT_COMMAND="__agent_terminal_precmd
$PROMPT_COMMAND
__agent_terminal_ready"
else
  PROMPT_COMMAND="__agent_terminal_precmd
__agent_terminal_ready"
fi
trap __agent_terminal_preexec DEBUG
