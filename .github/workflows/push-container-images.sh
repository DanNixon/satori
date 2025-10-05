#!/usr/bin/env bash

set -eo pipefail

# See https://github.com/actions/runner-images/issues/10443
sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0

local_cr="containers-storage:localhost/$PACKAGE:latest"

# The container registry to push images to
remote_cr="docker://ghcr.io/$GITHUB_REPOSITORY_OWNER/$PACKAGE"
remote_cr_creds="$GITHUB_REPOSITORY_OWNER:$GITHUB_TOKEN"

# Push image using the Git ref name as the image tag (i.e. "main" or the tag name)
skopeo copy --dest-creds="$remote_cr_creds" "$local_cr" "$remote_cr:$GITHUB_REF_NAME"

# Push image using the Git SHA as the image tag
skopeo copy --dest-creds="$remote_cr_creds" "$local_cr" "$remote_cr:$GITHUB_SHA"

# If the trigger was a tag (i.e. a release)
if [[ "$GITHUB_REF_TYPE" == 'tag' ]]; then
  skopeo copy --dest-creds="$remote_cr_creds" "$local_cr" "$remote_cr:latest"
fi
