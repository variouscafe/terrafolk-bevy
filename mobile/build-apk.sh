#!/bin/bash
# Terrafolk 안드로이드 APK 원스텝 빌드 — xbuild + 에셋 주입 + 정렬 + 서명.
# 함정 메모: 시스템 gradle 9는 xbuild 플러그인과 불호환(gradle@8 사용),
# NDK llvm-ar 경로 주입 필수, manifest의 assets는 gradle 모드에서 무시됨.
set -euo pipefail
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
export PATH="/opt/homebrew/opt/gradle@8/bin:$HOME/.cargo/bin:$PATH"
export ANDROID_HOME=${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}
export ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-$ANDROID_HOME/ndk/28.2.13676358}
export NDK_HOME=$ANDROID_NDK_HOME
export PATH="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/darwin-x86_64/bin:$PATH"

echo '[1/4] xbuild (rust+gradle apk)'
x build --platform android --arch arm64 --release --format apk
APK=$ROOT/target/x/release/android/mobile.apk

echo '[2/4] 에셋 주입 (manifest assets는 gradle에서 무시되므로 직접)'
cp "$APK" /tmp/terrafolk_inject.apk
(cd "$ROOT" && zip -qr /tmp/terrafolk_inject.apk assets)

BT=$ANDROID_HOME/build-tools/35.0.0
echo '[3/4] zipalign'
$BT/zipalign -f 4 /tmp/terrafolk_inject.apk /tmp/terrafolk_aligned.apk

echo '[4/4] debug 서명 (배포 서명은 release keystore로 교체할 것)'
$BT/apksigner sign --ks ~/.android/debug.keystore --ks-pass pass:android \
  --key-pass pass:android --out "$ROOT/Terrafolk.apk" /tmp/terrafolk_aligned.apk
echo "완료: $ROOT/Terrafolk.apk ($(du -h "$ROOT/Terrafolk.apk" | cut -f1))"
