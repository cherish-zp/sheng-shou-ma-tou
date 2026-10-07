#!/usr/bin/env bash
# 把 GitHub Release 的附件同步到 Gitee Release（CI 中 sync-gitee job 的本地兜底）。
#
# 用法：
#   GITEE_TOKEN=<gitee私人令牌> ./scripts/sync-gitee-release.sh v0.1.0
#
# 前置：本机已安装 gh 并登录（读取 GitHub Release 与附件）。
# 行为与 CI 一致：Gitee Release 存在则复用，同名附件跳过，单附件重试 4 次。
set -euo pipefail

TAG="${1:?usage: GITEE_TOKEN=... $0 v0.1.0}"
GITEE_TOKEN="${GITEE_TOKEN:?GITEE_TOKEN is required}"
GITHUB_REPO="${GITHUB_REPO:-cherish-zp/sheng-shou-ma-tou}"
GITEE_OWNER="${GITEE_OWNER:-princess-zp}"
GITEE_REPO="${GITEE_REPO:-sheng-shou-ma-tou}"

API="https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}"

# 1) GitHub Release 正文
BODY=$(gh release view "$TAG" --repo "$GITHUB_REPO" --json body --jq .body)

# 2) Gitee Release：存在则复用，不存在则创建
RELEASE_ID=$(curl -sf --connect-timeout 20 \
  "https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}/releases/tags/${TAG}?access_token=${GITEE_TOKEN}" \
  | jq -r '.id // empty' || true)
if [ -z "${RELEASE_ID}" ]; then
  echo "Creating Gitee release ${TAG}..."
  PAYLOAD=$(jq -n --arg tag "$TAG" --arg body "$BODY" \
    '{tag_name: $tag, name: ("Pier " + $tag), body: $body, prerelease: false, target_commitish: "main"}')
  RELEASE_ID=$(curl -sf --connect-timeout 20 -X POST "${API}/releases?access_token=${GITEE_TOKEN}" \
    -H 'Content-Type: application/json' -d "$PAYLOAD" | jq -r '.id')
fi
echo "Gitee release id: ${RELEASE_ID}"

# 3) 下载 GitHub Release 全部附件
rm -rf dist-gitee-sync && mkdir -p dist-gitee-sync
gh release download "$TAG" --repo "$GITHUB_REPO" --dir dist-gitee-sync

# 4) 幂等上传
EXISTING=$(curl -sf "${API}/releases/${RELEASE_ID}/attach_files?access_token=${GITEE_TOKEN}" \
  | jq -r '.[].browser_download_url' | xargs -r -n1 basename || true)
for f in dist-gitee-sync/*; do
  BASE=$(basename "$f")
  NAME="圣手码头${BASE#ShengShouMaTou}"
  if [ "$NAME" != "$BASE" ]; then mv "$f" "dist-gitee-sync/$NAME"; f="dist-gitee-sync/$NAME"; fi
  if echo "$EXISTING" | grep -qx "$NAME"; then
    echo "skip existing: $NAME"; continue
  fi
  for i in 1 2 3 4; do
    echo "upload $NAME (attempt $i)..."
    if curl -sf --connect-timeout 20 --max-time 1800 \
         -X POST "${API}/releases/${RELEASE_ID}/attach_files?access_token=${GITEE_TOKEN}" \
         -F "file=@${f}"; then
      echo "uploaded: $NAME"; break
    fi
    [ "$i" = 4 ] && echo "ERROR: failed to upload $NAME to Gitee" && exit 1
    sleep 30
  done
done
echo "Gitee sync done."
