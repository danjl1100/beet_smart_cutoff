cargo b && \
  BEET_COMMAND=./beet \
  TIMELESS_ARGS=grouping::'^(3|4|5)$' \
  OUTPUT_FILE=out.json \
  OUTPUT_KEY=key1 \
  ./target/debug/beet_smart_cutoff && \
  echo "" && \
  echo "cat out.json" && \
  cat out.json
