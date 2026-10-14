typeset -g __agent_terminal_running=

__agent_terminal_encode() {
  emulate -L zsh
  local LC_ALL=C text=$1 out= byte code
  local -i i
  for (( i = 1; i <= ${#text}; i++ )); do
    byte=${text[i]}
    case $byte in
      ([A-Za-z0-9/._~-]) out+=$byte ;;
      (*) printf -v code '%%%02X' "'$byte"; out+=$code ;;
    esac
  done
  REPLY=$out
}

__agent_terminal_precmd() {
  local code=$?
  emulate -L zsh
  if [[ -n $__agent_terminal_running ]]; then
    __agent_terminal_running=
    printf '\e]133;D;%s\a' "$code"
  fi
  __agent_terminal_encode "$PWD"
  printf '\e]133;A\a\e]7;file://%s\a\e]2;%s\a' "$REPLY" "${PWD//[[:cntrl:]]/ }"
  return $code
}

__agent_terminal_preexec() {
  emulate -L zsh
  __agent_terminal_running=1
  __agent_terminal_encode "$1"
  printf '\e]133;C;cmdline_url=%s\a\e]2;%s\a' "$REPLY" "${1//[[:cntrl:]]/ }"
}

__agent_terminal_line_init() {
  printf '\e]133;B\a'
}

__agent_terminal_install() {
  emulate -L zsh
  precmd_functions=(__agent_terminal_precmd ${precmd_functions:#__agent_terminal_install})
  preexec_functions+=(__agent_terminal_preexec)
  autoload -Uz add-zle-hook-widget
  zle -N __agent_terminal_line_init
  add-zle-hook-widget line-init __agent_terminal_line_init
  PROMPT_EOL_MARK=$'%{\e]133;D;%?\a%}'${PROMPT_EOL_MARK-%B%S%#%s%b}
  __agent_terminal_precmd
}

precmd_functions+=(__agent_terminal_install)
