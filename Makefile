APP       := mdiew
BUNDLE_ID := com.sho.mdiew
APP_DIR   := /Applications/$(APP).app
CONTENTS  := $(APP_DIR)/Contents
LSREGISTER := /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister

.PHONY: build install uninstall default clean

build:
	cargo build --release

install: build
	@echo "Installing $(APP).app..."
	rm -rf "$(APP_DIR)"
	mkdir -p "$(CONTENTS)/MacOS" "$(CONTENTS)/Resources"
	cp target/release/$(APP) "$(CONTENTS)/MacOS/$(APP)"
	cp bundle/Info.plist "$(CONTENTS)/Info.plist"
	cp assets/icon.icns "$(CONTENTS)/Resources/icon.icns"
	codesign --force --sign - "$(APP_DIR)"
	$(LSREGISTER) -f "$(APP_DIR)"
	@echo "Installed to $(APP_DIR)"

# Set mdiew as the default viewer for all markdown extensions.
# Requires: brew install duti
default: install
	duti -s $(BUNDLE_ID) .md all
	duti -s $(BUNDLE_ID) .markdown all
	duti -s $(BUNDLE_ID) .mdown all
	duti -s $(BUNDLE_ID) .mkd all
	duti -s $(BUNDLE_ID) .mkdn all
	@echo "mdiew is now the default app for .md files."

uninstall:
	rm -rf "$(APP_DIR)"
	$(LSREGISTER) -u "$(APP_DIR)" 2>/dev/null || true
	@echo "Uninstalled."

clean:
	cargo clean
