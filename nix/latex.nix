{ pkgs }:
let
  latexPackages = pkgs.texlive.combine {
    inherit (pkgs.texlive)
      scheme-small
      latexmk
      biblatex
      biber
      csquotes
      polyglossia
      geometry
      hyperref
      amsmath
      mathtools
      lualatex-math
      ;
  };
in
{
  devShell = pkgs.mkShell {
    packages = [
      latexPackages
      pkgs.fontconfig

      pkgs.texlab
      pkgs.pandoc
    ];
  };
}
