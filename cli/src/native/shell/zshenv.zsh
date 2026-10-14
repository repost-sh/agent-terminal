__agent_terminal_dir=$ZDOTDIR
if [[ -n ${AGENT_TERMINAL_ZDOTDIR+set} ]]; then
  ZDOTDIR=$AGENT_TERMINAL_ZDOTDIR
  unset AGENT_TERMINAL_ZDOTDIR
else
  unset ZDOTDIR
fi
if [[ -f ${ZDOTDIR:-$HOME}/.zshenv ]]; then
  source "${ZDOTDIR:-$HOME}/.zshenv"
fi
if [[ -o interactive ]]; then
  source "$__agent_terminal_dir/agent-terminal.zsh"
fi
unset __agent_terminal_dir
