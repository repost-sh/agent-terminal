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

__agent_terminal_ready() {
  __agent_terminal_armed=1
  case $PS1 in
    *$'\e]133;B\a'*) ;;
    *) PS1=$PS1$'\[\e]133;B\a\]' ;;
  esac
  return $__agent_terminal_code
}

__agent_terminal_preexec() {
  [ -n "$__agent_terminal_armed" ] || return 0
  __agent_terminal_armed=
  [ "$BASH_COMMAND" = __agent_terminal_precmd ] && return 0
  [ -n "${COMP_LINE-}" ] && return 0
  __agent_terminal_running=1
  local line pattern='^ *[0-9]+[* ] (.*)$'
  line=$(HISTTIMEFORMAT= builtin history 1)
  if [[ $line =~ $pattern ]]; then
    line=${BASH_REMATCH[1]}
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
