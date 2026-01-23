{ lib, buildGoModule, fetchFromGitHub, git }:

buildGoModule rec {
  pname = "git-wt";
  version = "0.15.0";

  src = fetchFromGitHub {
    owner = "k1LoW";
    repo = "git-wt";
    rev = "v${version}";
    hash = "sha256-A8vkwa8+RfupP9UaUuSVjkt5HtWvqR5VmSsVg2KpeMo=";
  };

  vendorHash = "sha256-K5geAvG+mvnKeixOyZt0C1T5ojSBFmx2K/Msol0HsSg=";

  nativeCheckInputs = [ git ];

  ldflags = [
    "-s"
    "-w"
    "-X github.com/k1LoW/git-wt/version.Version=${version}"
  ];

  # Skip tests that require git repo setup
  doCheck = false;

  meta = with lib; {
    description = "A Git subcommand that makes git worktree simple";
    homepage = "https://github.com/k1LoW/git-wt";
    license = licenses.mit;
    maintainers = [ ];
    mainProgram = "git-wt";
  };
}
