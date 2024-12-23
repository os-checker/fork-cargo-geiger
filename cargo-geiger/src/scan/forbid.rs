mod table;

use crate::format::print_config::{OutputFormat, PrintConfig};
use crate::graph::Graph;
use crate::mapping::CargoMetadataParameters;

use super::find::find_unsafe;
use super::{package_metrics, ScanMode, ScanParameters, ScanResult};

use table::scan_forbid_to_table;

use cargo::{CliError, GlobalContext as Config};
use cargo_geiger_serde::{QuickReportEntry, QuickSafetyReport};
use cargo_metadata::PackageId;

pub fn scan_forbid_unsafe(
    cargo_metadata_parameters: &CargoMetadataParameters,
    graph: &Graph,
    root_package_id: PackageId,
    scan_parameters: &ScanParameters,
) -> Result<ScanResult, CliError> {
    match scan_parameters.args.output_format {
        OutputFormat::Json => scan_forbid_to_report(
            cargo_metadata_parameters,
            scan_parameters.config,
            graph,
            scan_parameters.args.output_format,
            scan_parameters.print_config,
            root_package_id,
        ),
        _ => scan_forbid_to_table(
            cargo_metadata_parameters,
            scan_parameters.config,
            graph,
            scan_parameters.print_config,
            root_package_id,
        ),
    }
}

fn scan_forbid_to_report(
    cargo_metadata_parameters: &CargoMetadataParameters,
    config: &Config,
    graph: &Graph,
    output_format: OutputFormat,
    print_config: &PrintConfig,
    root_package_id: PackageId,
) -> Result<ScanResult, CliError> {
    let geiger_context = find_unsafe(
        cargo_metadata_parameters,
        config,
        ScanMode::EntryPointsOnly,
        print_config,
    )?;
    let mut report = QuickSafetyReport::default();
    for (package, package_metrics) in package_metrics(
        cargo_metadata_parameters,
        &geiger_context,
        graph,
        root_package_id,
    ) {
        let pack_metrics = match package_metrics {
            Some(m) => m,
            None => {
                report.packages_without_metrics.insert(package.id);
                continue;
            }
        };
        let forbids_unsafe = pack_metrics.rs_path_to_metrics.iter().all(
            |(_, rs_file_metrics_wrapper)| {
                rs_file_metrics_wrapper.metrics.forbids_unsafe
            },
        );
        let entry = QuickReportEntry {
            package,
            forbids_unsafe,
        };
        report.packages.insert(entry.package.id.clone(), entry);
    }
    let json_string = match output_format {
        OutputFormat::Json => serde_json::to_string(&report).unwrap(),
        _ => panic!("Only implemented for OutputFormat::Json"),
    };

    Ok(ScanResult {
        scan_output_lines: vec![json_string],
        warning_count: 0,
    })
}
