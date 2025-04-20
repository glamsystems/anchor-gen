GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

cargo run -q -p \
    glam-cpi-gen remapping ../../glam/anchor/deps/drift/drift.json \
    --config ../../glam/anchor/programs/glam_protocol/src/cpi_autogen/config.yaml \
    --program-id dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH \
    --output /tmp/remapping.json \
    --ixs initializeUser \
    --ixs logUserBalances # not configured, no proxy

diff /tmp/remapping.json ./drift-remapping-expected.json > /dev/null

if [ $? -ne 0 ]; then
    echo "${RED}❌ Test failed"
    echo "📊 Diff between generated and expected:"
    diff /tmp/remapping.json ./drift-remapping-expected.json
else
    echo "${GREEN}✅ Test passed"
fi