$pdf_mode = 4;
$postscript_mode = $dvi_mode = 0;

$pdf_previewer = 'open -a Skim';
$pdflatex = 'pdflatex -synctex=1 -interaction=nonstopmode';
@generated_exts = (@generated_exts, 'synctex.gz');

$warnings_as_errors = 0;
$show_time = 1;

$bibtex_use = 1;
$biber = "biber --validate-datamodel %O %S";

$out_dir = "target";

push @generated_exts, 'loe', 'lol', 'run.xml', 'glg', 'glstex';
$clean_ext = "%R-*.glstex %R_contourtmp*.*";

$pvc_view_file_via_temporary = 0;

# Optional: make output directory if missing
system("mkdir -p $out_dir");
system("mkdir -p $out_dir/chapters");
