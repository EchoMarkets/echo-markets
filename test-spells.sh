#!/bin/bash
# =============================================================================
# TEST SPELLS WITH CHARMS CLI
# =============================================================================
# Both `charms spell check` and `charms spell prove` require prev_txs
# (actual transaction data from blockchain).
#
# This script has TWO modes:
#
# 1. DRY RUN (default): Shows substituted spells, validates YAML structure
#    ./test-spells.sh
#
# 2. REAL TEST: Tests with actual testnet4 UTXOs (you must provide them)
#    ./test-spells.sh --real
#
# For quick YAML validation without CLI, use: ./validate-spells.sh
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SPELLS_DIR="${SCRIPT_DIR}/spells"

# Parse arguments
APP_WASM="./target/wasm32-wasip1/release/echo-markets.wasm"
MODE="dry"

for arg in "$@"; do
    case $arg in
        --real)
            MODE="real"
            ;;
        *.wasm)
            APP_WASM="$arg"
            ;;
    esac
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check WASM exists
if [ ! -f "$APP_WASM" ]; then
    echo -e "${RED}❌ Contract WASM not found: $APP_WASM${NC}"
    echo ""
    echo "Build your contract first:"
    echo "  cargo build --release --target wasm32-wasip1"
    exit 1
fi

# Get app VK
# Use environment variable if set, otherwise use hardcoded default
if [ -z "$APP_VK" ]; then
    APP_VK="your_app_vk_here"
    
    # Try to get from charms command if available (only if not set via env)
    if command -v charms &> /dev/null; then
        VK_FROM_CMD=$(charms app vk --wasm "$APP_WASM" 2>/dev/null | grep -oE '[0-9a-f]{64}' | head -1)
        if [ -n "$VK_FROM_CMD" ]; then
            APP_VK="$VK_FROM_CMD"
        fi
    fi
fi

if [ -z "$APP_VK" ]; then
    APP_VK="0000000000000000000000000000000000000000000000000000000000000000"
    echo -e "${YELLOW}⚠️  Could not get app VK (using placeholder)${NC}"
fi

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Spell Testing"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "   WASM: $APP_WASM"
echo "   App VK: ${APP_VK:0:16}..."
echo ""

# =============================================================================
# EXPORT ALL TEST VARIABLES
# =============================================================================

export app_vk="$APP_VK"
export in_utxo_0="${IN_UTXO_0:-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:0}"
export market_utxo_id="${MARKET_UTXO_ID:-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:0}"
export user_btc_utxo="${USER_BTC_UTXO:-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc:0}"
export user_tokens_utxo="${USER_TOKENS_UTXO:-dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd:0}"
export user_yes_utxo="${USER_YES_UTXO:-eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee:0}"
export user_no_utxo="${USER_NO_UTXO:-ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff:0}"
export alice_yes_utxo="${ALICE_YES_UTXO:-1111111111111111111111111111111111111111111111111111111111111111:0}"
export bob_no_utxo="${BOB_NO_UTXO:-2222222222222222222222222222222222222222222222222222222222222222:0}"

# Detect hash command
HASH_CMD="sha256sum"
if ! command -v sha256sum &> /dev/null; then
    HASH_CMD="shasum -a 256"
fi

export market_id=$(echo -n "${in_utxo_0}" | $HASH_CMD | cut -d' ' -f1)
export question_hash=$(echo -n "Will BTC reach 100k?" | $HASH_CMD | cut -d' ' -f1)
export yes_token_id=$(echo -n "${market_id}YES" | $HASH_CMD | cut -d' ' -f1)
export no_token_id=$(echo -n "${market_id}NO" | $HASH_CMD | cut -d' ' -f1)

export trading_deadline=1735603200
export resolution_deadline=1735689600
export fee_bps=100
export min_bet=10000
export max_supply=1000000000000

export creator_pubkey="020000000000000000000000000000000000000000000000000000000000000001"
export resolver_pubkey="020000000000000000000000000000000000000000000000000000000000000002"
export resolution_signature="00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001"
export cancel_signature="00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002"
export creator_cancel_signature="${cancel_signature}"
export creator_signature="${cancel_signature}"

export addr_0="tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c"
export addr_market="tb1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcwrs20vcsfgtyxqkmqarqy9y2n"
export addr_user="tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c"
export addr_creator="tb1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcwrs20vcsfgtyxqkmqarqy9y2n"
export addr_alice="tb1pqqqqp399et2xygdj5xreqhjjvcmzhxw4aywxecjdzew6hylgvsesf3hn0c"
export addr_bob="tb1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcwrs20vcsfgtyxqkmqarqy9y2n"

export current_timestamp=1735500000
export resolution_timestamp=${current_timestamp}
export cancel_timestamp=${current_timestamp}
export current_status="Active"
export outcome="Yes"

export old_yes_supply=0
export old_no_supply=0
export old_fees=0
export accumulated_fees=10000
export yes_supply=990000
export no_supply=990000

export mint_amount=1000000
export fee=$((mint_amount * fee_bps / 10000))
export shares=$((mint_amount - fee))
export new_yes_supply=$((old_yes_supply + shares))
export new_no_supply=$((old_no_supply + shares))
export new_fees=$((old_fees + fee))

export burn_amount=100000
export yes_tokens_burned=990000
export no_tokens_burned=0
export trade_amount=50000

# Spell list
SPELLS=(
    "create-market"
    "mint-shares"
    "burn-shares"
    "trade"
    "resolve-market"
    "redeem"
    "cancel-market"
    "claim-fees"
)

# =============================================================================
# DRY RUN MODE - Just validate YAML and show substituted output
# =============================================================================

if [ "$MODE" != "real" ]; then
    echo -e "${BLUE}📋 DRY RUN MODE${NC}"
    echo ""
    echo "This validates YAML structure and variable substitution."
    echo "It does NOT run 'charms spell check' (which requires prev_txs)."
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    
    PASSED=0
    FAILED=0
    
    for spell in "${SPELLS[@]}"; do
        spell_file="$SPELLS_DIR/${spell}.yaml"
        echo -n "  Checking ${spell}... "
        
        if [ ! -f "$spell_file" ]; then
            echo -e "${RED}NOT FOUND${NC}"
            ((FAILED++))
            continue
        fi
        
        # Substitute and check for remaining variables
        substituted=$(cat "$spell_file" | envsubst)
        unsubstituted=$(echo "$substituted" | grep -oE '\$\{[^}]+\}' | sort -u || true)
        
        if [ -n "$unsubstituted" ]; then
            echo -e "${YELLOW}MISSING VARS${NC}"
            echo "$unsubstituted" | while read var; do echo "      $var"; done
            ((FAILED++))
            continue
        fi
        
        # Check required sections
        has_version=$(echo "$substituted" | grep -c "^version:" || true)
        has_apps=$(echo "$substituted" | grep -c "^apps:" || true)
        has_ins=$(echo "$substituted" | grep -c "^ins:" || true)
        has_outs=$(echo "$substituted" | grep -c "^outs:" || true)
        
        if [ "$has_version" -eq 0 ] || [ "$has_apps" -eq 0 ] || [ "$has_ins" -eq 0 ] || [ "$has_outs" -eq 0 ]; then
            echo -e "${RED}MISSING SECTIONS${NC}"
            ((FAILED++))
            continue
        fi
        
        # Validate YAML syntax
        if command -v python3 &> /dev/null; then
            if ! echo "$substituted" | python3 -c "import sys, yaml; yaml.safe_load(sys.stdin)" 2>/dev/null; then
                echo -e "${RED}INVALID YAML${NC}"
                ((FAILED++))
                continue
            fi
        fi
        
        echo -e "${GREEN}✅ OK${NC}"
        ((PASSED++))
    done
    
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo -e "Results: ${GREEN}${PASSED} passed${NC}, ${RED}${FAILED} failed${NC}"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""
    echo "⚠️  This only validates YAML structure, not contract logic."
    echo ""
    echo "To test with real blockchain data:"
    echo "  1. Get testnet4 UTXOs"
    echo "  2. Set IN_UTXO_0 and PREV_TX environment variables"
    echo "  3. Run: ./test-spells.sh --real"
    echo ""
    echo "Or use the v8 Prover API directly with real UTXOs."
    
    exit $FAILED
fi

# =============================================================================
# REAL TEST MODE - Requires actual testnet4 UTXOs
# =============================================================================

echo -e "${YELLOW}🔥 REAL TEST MODE${NC}"
echo ""

if ! command -v charms &> /dev/null; then
    echo -e "${RED}❌ Charms CLI not found${NC}"
    echo "  cargo install --git https://github.com/CharmsDev/charms"
    exit 1
fi

# Check if user has set real UTXOs
if [[ "$in_utxo_0" == *"aaaaaaa"* ]]; then
    echo -e "${RED}❌ You're using placeholder UTXOs${NC}"
    echo ""
    echo "To test with real UTXOs:"
    echo ""
    echo "1. Get a testnet4 UTXO from your wallet or a faucet"
    echo ""
    echo "2. Fetch the prev_tx:"
    echo "   TXID=your_transaction_id"
    echo "   PREV_TX=\$(curl -s https://mempool.space/testnet4/api/tx/\$TXID/hex)"
    echo ""
    echo "3. Run with environment variables:"
    echo "   IN_UTXO_0=\"\$TXID:0\" PREV_TX=\"\$PREV_TX\" ./test-spells.sh --real"
    echo ""
    exit 1
fi

if [ -z "$PREV_TX" ]; then
    echo -e "${RED}❌ PREV_TX environment variable not set${NC}"
    echo ""
    echo "Fetch it with:"
    echo "  TXID=\"${in_utxo_0%:*}\""
    echo "  export PREV_TX=\$(curl -s https://mempool.space/testnet4/api/tx/\$TXID/hex)"
    exit 1
fi

echo "Testing create-market with real UTXO..."
echo "   UTXO: $in_utxo_0"
echo ""

# Create substituted spell
temp_spell=$(mktemp)
cat "$SPELLS_DIR/create-market.yaml" | envsubst > "$temp_spell"

echo "Running: charms spell check --prev-txs ..."
echo ""

if charms spell check --spell "$temp_spell" --app-bins "$APP_WASM" --prev-txs "$PREV_TX" 2>&1; then
    echo ""
    echo -e "${GREEN}✅ create-market PASSED${NC}"
else
    echo ""
    echo -e "${RED}❌ create-market FAILED${NC}"
fi

rm -f "$temp_spell"
