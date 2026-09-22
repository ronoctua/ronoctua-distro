# THE SOFTWARE RASTERISERS, named once.
#
# Two of them, for two processes: `lavapipe` — Mesa's software Vulkan — is what
# the footer's `wgpu` draws with, and `pixman` is what the compositor
# composites with in a machine that has no GPU at all.
#
# Section 9.3 chose software rendering for the tests: hermetic, deterministic,
# runs with no card. And it made the other side explicit — the performance
# targets of section 5.2 are NEVER measured through any of this.
#
# # Why it is a file of its own
#
# Because three of its four consumers are not tests. `nix run .#png` draws
# without a compositor at all, and two checks reach for the same path. It used
# to live inside the compositor's test prelude, which made deleting that file
# break three unrelated things.
{ pkgs, lib }:

rec {
  # Mesa's software Vulkan. The footer asks for Vulkan and gets this.
  icd = "${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json";

  # What a process needs to find the software stacks.
  #
  # `WLR_RENDERER=pixman` IS THE COMPOSITOR'S HALF, and it is worth saying why
  # it and not software GLES: `pixman` is a CPU rasteriser with no driver under
  # it, so it opens no DRM node, never touches EGL and never reaches GBM. That
  # removes the whole class of "a path compiled for NixOS does not exist here"
  # from the test machines — and it is why `WLR_RENDERER_ALLOW_SOFTWARE` is
  # deliberately NOT set: that variable is read on the GLES path only, so
  # setting it would be a variable that does nothing.
  env = lib.concatStringsSep " " [
    "--setenv=WLR_RENDERER=pixman"
    "--setenv=VK_DRIVER_FILES=${icd}"
    # The older name, for a loader that has not learned the new one.
    "--setenv=VK_ICD_FILENAMES=${icd}"
    "--setenv=LIBGL_ALWAYS_SOFTWARE=1"
  ];

  # What has to be installed for the above to resolve.
  packages = [ pkgs.mesa ];
}
