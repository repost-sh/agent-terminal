if status is-interactive
    set -g __agent_terminal_running

    function __agent_terminal_prompt --on-event fish_prompt
        printf '\e]133;A\a\e]7;file://%s\a' (string escape --style=url -- $PWD)
    end

    function __agent_terminal_preexec --on-event fish_preexec
        set -g __agent_terminal_running 1
        printf '\e]133;C;cmdline_url=%s\a' (string escape --style=url -- $argv[1])
    end

    function __agent_terminal_postexec --on-event fish_postexec
        set -l code $status
        if set -q __agent_terminal_running[1]
            set -g __agent_terminal_running
            printf '\e]133;D;%s\a' $code
        end
    end

    if functions -q fish_prompt
        functions -c fish_prompt __agent_terminal_user_prompt
    else
        function __agent_terminal_user_prompt
        end
    end

    function fish_prompt
        __agent_terminal_user_prompt
        printf '\e]133;B\a'
    end
end
