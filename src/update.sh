#!/usr/bin/env bash
# This file is generated by fork-manager. Do not edit by hand !
set -xeuo pipefail

# https://stackoverflow.com/a/246128/1368502
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

pushd "$SCRIPT_DIR"

git submodule update --init

#{ for fork in forks }#
# '{ fork.name }' {{

if git submodule status | grep -q " '{ fork.name }' "
then
    git submodule set-branch --branch '{ fork.target.branch }' '{ fork.name }'
    git submodule set-url '{ fork.name }' '{ fork.target.url }'
else
    git submodule add -b '{ fork.target.branch }' '{ fork.target.url }' '{ fork.name }'
    git add .gitmodules
    git commit -m "add submodule for '{ fork.name }'"
fi

pushd '{ fork.name }'
git checkout '{ fork.target.branch }'

#/*
# shellcheck disable=SC2041
#*/
for remote in '{ remotes }'
do
    name=$(echo "$remote" | sed 's=https://==;s=git@==;s=:=/=g')
    if git remote | grep -q "^$name$"
    then
        git remote set-url "$name" "$remote"
    else
        git remote add "$name" "$remote"
    fi
done

git fetch --all --prune
git reset --hard '{ fork.upstream.url | remote_name }'/'{ fork.upstream.branch }'

{
    echo "# fork-manager"
    echo
#{ if config }#
    echo "This branch is managed from '{ config.url }'"
#{ if config.branch }#
    echo "on '{ config.branch }'"
#{ endif }#
#{ else }#
    echo "This branch is managed with [fork-manager](https://github.com/nim65s/fork-manager/)."
#{ endif }#

    echo "It is based on '{ fork.upstream.url }' '{ fork.upstream.branch }'"
    echo -n "  which is on: "
    git log --no-color --format=reference -1

    echo
    echo "It include:"

#{ for change in fork.changes }#
# '{ change.title }' {{{
    git merge --no-edit "'{ change.url | remote_name }'/'{ change.branch }'" >&2
    echo "- '{ change.title }'"
    echo -n "  in: "
    git log --no-color --format=reference -1
    echo -n "  which is on: "
    git log --no-color --format=reference -1 "'{ change.url | remote_name }'/'{ change.branch }'"
# }}}
#{ endfor }#

    echo
    echo "---"
    echo

    test -f README.md && cat README.md
} > "../.'{ fork.name }'.log"

mv "../.'{ fork.name }'.log" README.md
git add README.md
git commit -m "document fork manager"

[[ -x "../test-'{ fork.name }'.sh" ]] && "../test-'{ fork.name }'.sh"

[[ "$#" -eq 1 && "$1" == "push" ]] && git push -f '{ fork.target.url | remote_name }' '{ fork.target.branch }'
popd

git add '{ fork.name }'
git commit -m "updated '{ fork.name }'"

# }}
#{ endfor }#

popd
