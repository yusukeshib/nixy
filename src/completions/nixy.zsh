# nixy zsh completion
# Dynamic completion candidates are produced by the hidden `nixy completions`
# helper, so installed package names and profiles always stay in sync.

__nixy_installed() {
    local -a pkgs
    pkgs=(${(f)"$(command nixy completions installed 2>/dev/null)"})
    if (( ${#pkgs} )); then
        _describe 'package' pkgs
    fi
}

__nixy_profiles() {
    local -a profiles
    profiles=(${(f)"$(command nixy completions profiles 2>/dev/null)"})
    if (( ${#profiles} )); then
        _describe 'profile' profiles
    fi
}

_nixy() {
    local curcontext="$curcontext" state line
    typeset -A opt_args

    _arguments -C \
        '1: :->subcmd' \
        '*:: :->args'

    case $state in
        subcmd)
            local -a subcmds
            subcmds=(
                'install:Install a package from nixpkgs'
                'add:Install a package from nixpkgs (alias)'
                'uninstall:Uninstall a package'
                'remove:Uninstall a package (alias)'
                'list:List installed packages'
                'ls:List installed packages (alias)'
                'search:Search for packages'
                'update:Update packages and flake inputs'
                'sync:Build environment and create symlink'
                'config:Output shell configuration'
                'profile:Profile management'
                'upgrade:Upgrade nixy to the latest version'
                'file:Show path to a package source file'
            )
            _describe 'subcommand' subcmds
            ;;
        args)
            case $words[1] in
                install|add)
                    _arguments \
                        '*'{-p,--platform}'=[Restrict to platform(s)]:platform:(darwin macos linux x86_64-darwin aarch64-darwin x86_64-linux aarch64-linux)' \
                        '1:package:'
                    ;;
                uninstall|remove)
                    _arguments '1:package:__nixy_installed'
                    ;;
                update)
                    _arguments \
                        '--all[Update all packages and inputs]' \
                        '*:package:__nixy_installed'
                    ;;
                file)
                    _arguments '1:package:__nixy_installed'
                    ;;
                search)
                    _arguments '1:query:'
                    ;;
                config)
                    if (( CURRENT == 2 )); then
                        local -a shells
                        shells=(
                            'zsh:Zsh configuration'
                            'bash:Bash configuration'
                            'fish:Fish configuration'
                        )
                        _describe 'shell' shells
                    fi
                    ;;
                profile)
                    _arguments \
                        '-c[Create the profile if it does not exist]' \
                        '-d[Delete the specified profile]' \
                        '1:profile:__nixy_profiles'
                    ;;
                upgrade)
                    _arguments '(-f --force)'{-f,--force}'[Force reinstall even if already latest]'
                    ;;
            esac
            ;;
    esac
}

if typeset -f compdef >/dev/null; then
    compdef _nixy nixy
fi
