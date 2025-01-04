use rstest::*;

use bkmr::model::bookmark::Bookmark;
use bkmr::service::fzf::fzf_process;
use bkmr::util::testing::bms;

/// uses interactive console
#[rstest]
#[ignore = "Interactive via Makefile"]
fn given_bookmark_list_when_running_fzf_then_processes_interactively(bms: Vec<Bookmark>) {
    fzf_process(&bms);
}
