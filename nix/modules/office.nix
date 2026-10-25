# The office — ONLYOFFICE and LibreOffice, and which of the two opens what.
#
# WHAT IS DECLARED
# ----------------
# - The two suites, from nixpkgs, as programs of the registry in the `office`
#   group: `profile.nix` turns the group on in the full profile, and a person
#   may decline either (the installer's list leaves LibreOffice unmarked by
#   default, so a fresh full machine has ONLYOFFICE alone).
# - Which one OPENS a document, in `/etc/xdg/mimeapps.list`
#   (`xdg.mime.defaultApplications`, the file `workshop.nix` and
#   `transfers.nix` write too). Both suites claim both families, so without a
#   default the file manager would hand a document to whichever entry it
#   met first. The split:
#     * Microsoft's formats (OOXML: `.docx`, `.xlsx`, `.pptx`, their
#       templates and their macro-enabled kin; the binary `.doc`, `.xls` and
#       `.ppt` before them; and Word's `.rtf`) open in ONLYOFFICE, whose
#       whole engine is built around them; without it, in LibreOffice.
#     * The open formats (ODF: `.odt`, `.ods`, `.odp`, `.odg`, their
#       templates and flat XML) open in LibreOffice, ODF's own reference;
#       without it, in ONLYOFFICE.
#   Only the types BOTH suites claim are named: a type one suite alone
#   claims has one candidate, and a default would only repeat it.
#
# WHY THE DEFAULTS FOLLOW `installed`, NOT THE SWITCH
# ---------------------------------------------------
# A default naming an entry that is not there is skipped by every reader in
# silence (`ronoctua_apps::defaults`), so a person who declined ONLYOFFICE
# must find the Microsoft formats named to LibreOffice, not to a ghost. Each
# default is set from `ronoctua.distroApps.installed`, which reads the
# declines — the shape `workshop.nix` gives OrcaSlicer's model types.
#
# WHY THE ENTRIES' NAMES ARE WRITTEN, AND CHECKED
# -----------------------------------------------
# Upstream names them (`onlyoffice-desktopeditors.desktop`, LibreOffice's
# `writer.desktop`, `calc.desktop`…), and reading them out of the packages at
# evaluation would make every evaluation of a machine with the suites wait
# for two gigabytes to be fetched (import-from-derivation). So they are
# written here, and `system.build.officeEntries` reads each named entry: it
# must exist in the package, be an application, and claim the type it is
# named for. NOT a `system.checks`: both suites are PARTS
# (`nix/modules/parts.nix`), realized after the installation, and building a
# system — offline, from the ISO — must not wait for them;
# `documents-open-in-their-suite` builds the reading for each machine it
# asks about, and proves the whole way from a file to the entry, and
# `distro-apps-are-registered` that the registry's entries are there.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  installed = config.ronoctua.distroApps.installed;
  onlyoffice = pkgs.onlyoffice-desktopeditors;
  # `fresh` and not `still`: the newest features and filters, which is what
  # a person reaching for LibreOffice beside ONLYOFFICE comes for.
  libreoffice = pkgs.libreoffice-fresh;
  onlyofficeEntry = "onlyoffice-desktopeditors.desktop";

  # THE MICROSOFT FORMATS BOTH SUITES CLAIM, each with the LibreOffice entry
  # that claims it (measured in the pinned packages' `MimeType=`). The binary
  # three are here too, or one family would have two answers: the catalogue
  # is sorted by name, "LibreOffice Writer" comes before "ONLYOFFICE", and a
  # `.doc` would open in LibreOffice beside a `.docx` in ONLYOFFICE.
  ooxml = {
    "application/msword" = "writer.desktop";
    "application/vnd.ms-excel" = "calc.desktop";
    "application/vnd.ms-powerpoint" = "impress.desktop";
    "application/rtf" = "writer.desktop";
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" = "writer.desktop";
    "application/vnd.openxmlformats-officedocument.wordprocessingml.template" = "writer.desktop";
    "application/vnd.ms-word.document.macroEnabled.12" = "writer.desktop";
    "application/vnd.ms-word.template.macroEnabled.12" = "writer.desktop";
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" = "calc.desktop";
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template" = "calc.desktop";
    "application/vnd.ms-excel.sheet.macroEnabled.12" = "calc.desktop";
    "application/vnd.ms-excel.sheet.binary.macroEnabled.12" = "calc.desktop";
    "application/vnd.ms-excel.template.macroEnabled.12" = "calc.desktop";
    "application/vnd.openxmlformats-officedocument.presentationml.presentation" = "impress.desktop";
    "application/vnd.openxmlformats-officedocument.presentationml.slideshow" = "impress.desktop";
    "application/vnd.openxmlformats-officedocument.presentationml.template" = "impress.desktop";
    "application/vnd.ms-powerpoint.presentation.macroEnabled.12" = "impress.desktop";
    "application/vnd.ms-powerpoint.slideshow.macroEnabled.12" = "impress.desktop";
    "application/vnd.ms-powerpoint.template.macroEnabled.12" = "impress.desktop";
  };
  # THE OPEN FORMATS BOTH SUITES CLAIM, likewise.
  odf = {
    "application/vnd.oasis.opendocument.text" = "writer.desktop";
    "application/vnd.oasis.opendocument.text-template" = "writer.desktop";
    "application/vnd.oasis.opendocument.text-flat-xml" = "writer.desktop";
    "application/vnd.oasis.opendocument.spreadsheet" = "calc.desktop";
    "application/vnd.oasis.opendocument.spreadsheet-template" = "calc.desktop";
    "application/vnd.oasis.opendocument.spreadsheet-flat-xml" = "calc.desktop";
    "application/vnd.oasis.opendocument.presentation" = "impress.desktop";
    "application/vnd.oasis.opendocument.presentation-template" = "impress.desktop";
    "application/vnd.oasis.opendocument.presentation-flat-xml" = "impress.desktop";
    "application/vnd.oasis.opendocument.graphics" = "draw.desktop";
  };

  hasOnlyoffice = installed.onlyoffice;
  hasLibreoffice = installed.libreoffice;
  # Each type to the entry that opens it on THIS machine, with the package
  # that entry comes from — or no line when neither suite is here.
  ofOnlyoffice =
    types:
    lib.mapAttrs (_: _: {
      package = onlyoffice;
      entry = onlyofficeEntry;
    }) types;
  ofLibreoffice =
    types:
    lib.mapAttrs (_: entry: {
      package = libreoffice;
      inherit entry;
    }) types;
  defaults =
    (
      if hasOnlyoffice then
        ofOnlyoffice ooxml
      else if hasLibreoffice then
        ofLibreoffice ooxml
      else
        { }
    )
    // (
      if hasLibreoffice then
        ofLibreoffice odf
      else if hasOnlyoffice then
        ofOnlyoffice odf
      else
        { }
    );

  # EVERY NAMED ENTRY, read at build (see the header): there, an application,
  # claiming its type.
  entriesAreThere = pkgs.runCommand "office-entries" { } (
    lib.concatStrings (
      lib.mapAttrsToList (type: d: ''
        entry=${d.package}/share/applications/${d.entry}
        if [ ! -f "$entry" ]; then
          echo "ERROR: ${d.entry} is not in ${d.package.name}; these are:" >&2
          ls ${d.package}/share/applications >&2 || true
          exit 1
        fi
        grep -qx 'Type=Application' "$entry" || {
          echo "ERROR: ${d.entry} is not an application entry" >&2; exit 1; }
        grep '^MimeType=' "$entry" | tr ';=' '\n\n' | grep -qxF '${type}' || {
          echo "ERROR: ${d.entry} no longer opens ${type}" >&2; exit 1; }
      '') defaults
    )
    + ''
      touch $out
    ''
  );
in
{
  # The registry this module writes its two programs into (see `browser.nix`
  # for why it is imported here too).
  imports = [ ./distro-apps.nix ];

  options.ronoctua.office.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      ONLYOFFICE and LibreOffice, with ONLYOFFICE as the default for
      Microsoft's formats and LibreOffice for the open ones (each falling
      back to the other where one is declined). On in the full profile.
    '';
  };

  config = lib.mkMerge [
    {
      # THE TWO ARE PROGRAMS OF THE REGISTRY (`nix/modules/distro-apps.nix`),
      # OUTSIDE any `mkIf`: this module's switch says the machine has them,
      # and a person may decline either.
      #
      # ONLYOFFICE is `agpl3Plus` in the pinned nixpkgs, a build of upstream's
      # own `.deb` (`binaryNativeCode`) and not unfree: the unfree predicate
      # does not name it. Its bundled Qt draws through XWayland
      # (`QT_QPA_PLATFORM=xcb`, the package's own wrapper).
      #
      # BOTH ARE PARTS (`nix/modules/parts.nix`): gigabytes the full profile
      # carries, realized after the installation and out of the system's
      # closure. Heavy: the small parts arrive first.
      ronoctua.apps.onlyoffice = {
        package = onlyoffice;
        part = true;
        heavy = true;
        name = "ONLYOFFICE";
        group = "office";
        desktopIds = [ onlyofficeEntry ];
      };
      # LibreOffice's entries, the start centre first: the software view names
      # a program by the first entry it finds, and that one is "LibreOffice".
      # `xsltfilter.desktop` is hidden (`NoDisplay=true`) and still the
      # package's own: owned, so no file of the suite is left unaccounted.
      ronoctua.apps.libreoffice = {
        package = libreoffice;
        part = true;
        heavy = true;
        name = "LibreOffice";
        group = "office";
        desktopIds = [
          "startcenter.desktop"
          "writer.desktop"
          "calc.desktop"
          "impress.desktop"
          "draw.desktop"
          "math.desktop"
          "base.desktop"
          "xsltfilter.desktop"
        ];
      };
    }

    (lib.mkIf (hasOnlyoffice || hasLibreoffice) {
      xdg.mime.enable = true;
      xdg.mime.defaultApplications = lib.mapAttrs (_: d: d.entry) defaults;
      # The reading, by name, for `documents-open-in-their-suite`, which
      # builds it for each machine it asks about (see above: never a
      # `system.checks`).
      system.build.officeEntries = entriesAreThere;
    })
  ];
}
