$VERSION = "0.1.0"
$PLATFORM = "x64"

cargo build --bin gu-provider --release

# $env:Path += ";C:\Program Files (x86)\WiX Toolset v3.11\bin"
# $env:Path += ";C:\Users\imapp\Desktop"

candle.exe -dVersion=$VERSION -dPlatform=$Platform main.wxs

