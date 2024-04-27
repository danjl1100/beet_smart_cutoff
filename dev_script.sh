# TIMELESS_ARGS=grouping::'^(3|4|5)$' \
# declare -x -a TIMELESS_ARGS_LIST=(grouping::'^(3|4|5)$')
declare -x -a TIMELESS_ARGS_LIST=()
declare -x TIMELESS_ARGS="$(for f in "${TIMELESS_ARGS_LIST[@]}"; do
  echo "$f"
done)"

cargo b && \
  BEET_COMMAND=./beet \
  OUTPUT_FILE=out.json \
  OUTPUT_KEY=key1 \
  ./target/debug/beet_smart_cutoff && \
  echo "" && \
  echo "cat out.json" && \
  cat out.json
