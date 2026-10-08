#!/usr/bin/env bash
# 把 GitHub Release 的附件同步到 Gitee（CI sync-gitee job 的本地兜底）。
#
# 用法：
#   GITEE_TOKEN=<gitee私人令牌> ./scripts/sync-gitee-release.sh v0.3.0
#
# 前置：本机有 curl、python3（无需 gh/jq）。
# 为什么需要本地跑：Gitee 附件从 GitHub Actions（US runner）跨境上传
# 实测不可行（83MB AppImage 卡 3 小时 0%），CI 只传 <50MB 的小文件；
# 大附件从本机走境内链路补传，速度有保障。
#
# 行为：
#   1) 版本化 Release（vX.Y.Z）：存在则复用；只下载 Gitee 缺失的附件，
#      本机直传（无大小限制，受 Gitee 单附件 100MB 上限约束）；
#   2) 常驻 latest release（自动更新检查端点）：latest.json 重写为
#      Gitee 版本化 URL 后覆盖上传。
# 幂等：同名附件跳过，可重复执行。
set -euo pipefail

TAG="${1:?usage: GITEE_TOKEN=... $0 v0.3.0}"
GITEE_TOKEN="${GITEE_TOKEN:?GITEE_TOKEN is required}"
GITHUB_REPO="${GITHUB_REPO:-cherish-zp/sheng-shou-ma-tou}"
GITEE_OWNER="${GITEE_OWNER:-princess-zp}"
GITEE_REPO="${GITEE_REPO:-sheng-shou-ma-tou}"
export GITEE_OWNER GITEE_REPO

API="https://gitee.com/api/v5/repos/${GITEE_OWNER}/${GITEE_REPO}"
DISPLAY="圣手码头"
WORK="dist-gitee-sync"

# 1) GitHub Release 元数据
META=$(curl -sf --retry 3 "https://api.github.com/repos/${GITHUB_REPO}/releases/tags/${TAG}")
echo "$META" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("GitHub release:", d.get("name"), "| assets:", len(d.get("assets", [])))'
BODY=$(echo "$META" | python3 -c 'import json,sys; print(json.load(sys.stdin)["body"])')

# 2) Gitee 版本化 Release：存在则复用，不存在则创建
RELEASE_ID=$(curl -sf --connect-timeout 20 "${API}/releases/tags/${TAG}?access_token=${GITEE_TOKEN}" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin).get("id", ""))' || true)
if [ -z "${RELEASE_ID}" ]; then
  echo "Creating Gitee release ${TAG}..."
  PAYLOAD=$(python3 - "$TAG" "$BODY" <<'PY'
import json, sys
print(json.dumps({"tag_name": sys.argv[1], "name": "Pier " + sys.argv[1],
                  "body": sys.argv[2], "prerelease": False,
                  "target_commitish": "main"}))
PY
  )
  RELEASE_ID=$(curl -sf --connect-timeout 20 -X POST "${API}/releases?access_token=${GITEE_TOKEN}" \
    -H 'Content-Type: application/json' -d "$PAYLOAD" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
fi
echo "Gitee release id: ${RELEASE_ID}"

# 3) 幂等补传：只下载 Gitee 缺失的附件
rm -rf "$WORK" && mkdir -p "$WORK"
EXISTING=$(curl -sf "${API}/releases/${RELEASE_ID}/attach_files?access_token=${GITEE_TOKEN}&per_page=100" \
  | python3 -c 'import json,sys; [print(a["name"]) for a in json.load(sys.stdin)]' || true)

echo "$META" | python3 -c '
import json, sys
for a in json.load(sys.stdin)["assets"]:
    print(a["name"] + "\t" + a["browser_download_url"])' | while IFS=$'\t' read -r NAME URL; do
  GNAME="$NAME"
  case "$GNAME" in ShengShouMaTou*) GNAME="${DISPLAY}${GNAME#ShengShouMaTou}";; esac
  if printf '%s\n' "$EXISTING" | grep -qx "$GNAME"; then
    echo "skip existing: $GNAME"; continue
  fi
  echo "download $NAME ..."
  curl -sfL --retry 3 --max-time 1800 -o "$WORK/$GNAME" "$URL"
  echo "upload $GNAME ..."
  for i in 1 2 3 4 5; do
    # shellcheck disable=SC2086
    if curl -fS --connect-timeout 20 --max-time 3600 \
         -X POST "${API}/releases/${RELEASE_ID}/attach_files?access_token=${GITEE_TOKEN}" \
         -F "file=@${WORK}/${GNAME}"; then
      echo "uploaded: $GNAME"; break
    fi
    if [ "$i" = 5 ]; then echo "ERROR: failed to upload $GNAME to Gitee"; exit 1; fi
    sleep 15
  done
done

# 4) 常驻 latest release（自动更新检查端点）
LATEST_ID=$(curl -sf --connect-timeout 20 "${API}/releases/tags/latest?access_token=${GITEE_TOKEN}" \
  | python3 -c 'import json,sys; print(json.load(sys.stdin).get("id", ""))' || true)
if [ -z "$LATEST_ID" ]; then
  LATEST_ID=$(curl -sf --connect-timeout 20 -X POST "${API}/releases?access_token=${GITEE_TOKEN}" \
    -H 'Content-Type: application/json' \
    -d '{"tag_name":"latest","name":"Latest release (auto-update endpoint)","body":"自动更新检查端点——请勿手动编辑","prerelease":true,"target_commitish":"main"}' \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
fi
echo "latest release id: ${LATEST_ID}"

python3 - "$TAG" <<'PY'
import json, sys, urllib.parse, os
tag = sys.argv[1]
owner, repo = os.environ["GITEE_OWNER"], os.environ["GITEE_REPO"]
d = json.load(open("dist-gitee-sync/latest.json"))
base = f"https://gitee.com/{owner}/{repo}/releases/download/{tag}/"
for v in d.get("platforms", {}).values():
    name = urllib.parse.unquote(v["url"].rsplit("/", 1)[-1])
    if name.startswith("ShengShouMaTou"):
        name = "圣手码头" + name[len("ShengShouMaTou"):]
    v["url"] = base + urllib.parse.quote(name)
json.dump(d, open("dist-gitee-sync/latest.json", "w"), ensure_ascii=False, indent=2)
print("latest.json rewritten ->", base)
PY

OLD_LJ=$(curl -sf "${API}/releases/${LATEST_ID}/attach_files?access_token=${GITEE_TOKEN}&per_page=100" \
  | python3 -c 'import json,sys; print(" ".join(str(a["id"]) for a in json.load(sys.stdin) if a["name"] == "latest.json"))' || true)
for ID in $OLD_LJ; do
  curl -sf -X DELETE "${API}/releases/${LATEST_ID}/attach_files/${ID}?access_token=${GITEE_TOKEN}" || true
done
for i in 1 2 3 4 5; do
  if curl -fS --connect-timeout 20 --max-time 600 \
       -X POST "${API}/releases/${LATEST_ID}/attach_files?access_token=${GITEE_TOKEN}" \
       -F "file=@${WORK}/latest.json"; then
    echo "latest.json uploaded to Gitee latest release"; break
  fi
  if [ "$i" = 5 ]; then echo "ERROR: failed to upload latest.json to Gitee latest release"; exit 1; fi
  sleep 15
done

echo "Gitee sync done."
