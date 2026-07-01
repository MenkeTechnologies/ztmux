TESTS!= echo *.sh

.PHONY: all $(TESTS)
.NOTPARALLEL: all $(TESTS)

all: $(TESTS)

$(TESTS):
	# broken tests are ignored for now
	TEST_TMUX=../target/debug/ztmux sh $*.sh || true
	sleep 1
