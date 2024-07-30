use std::{fmt::Display, ops::Index};
use thiserror::Error;
use chrono::{DateTime, Utc};

const KEY_JNID: &str = "jnid";
const KEY_SALES_REP: &str = "sales_rep_name";
const KEY_INSURANCE_CHECKBOX: &str = "Insurance Job?";
const KEY_INSURANCE_COMPANY_NAME: &str = "Insurance Company";
const KEY_INSURANCE_CLAIM_NUMBER: &str = "Claim #";
const KEY_JOB_NUMBER: &str = "number";
const KEY_CUSTOMER_NAME: &str = "name";
const KEY_APPOINTMENT_DATE: &str = "Sales Appt #1 Date";
const KEY_CONTINGENCY_DATE: &str = "Signed Contingency Date";
const KEY_CONTRACT_DATE: &str = "Signed Contract Date";
const KEY_INSTALL_DATE: &str = "Install Date";
const KEY_LOSS_DATE: &str = "Loss Date (if applicable)";

pub type Timestamp = DateTime<Utc>;
pub type TimeDelta = chrono::TimeDelta;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Milestone {
    LeadAcquired,
    AppointmentMade,
    ContingencySigned,
    ContractSigned,
    Installed,
}
impl Milestone {
    pub const NUM_VARIANTS: usize = 5;

    pub fn ordered_iter() -> impl Iterator<Item = Self> {
        static ORDERED_VARIANTS: [Milestone; 5] = [
            Milestone::LeadAcquired,
            Milestone::AppointmentMade,
            Milestone::ContingencySigned,
            Milestone::ContractSigned,
            Milestone::Installed,
        ];
        ORDERED_VARIANTS.iter().copied()
    }

    pub const fn into_int(self) -> usize {
        match self {
            Milestone::LeadAcquired => 0,
            Milestone::AppointmentMade => 1,
            Milestone::ContingencySigned => 2,
            Milestone::ContractSigned => 3,
            Milestone::Installed => 4,
        }
    }
}
impl Display for Milestone {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Milestone::LeadAcquired => write!(f, "Lead Acquired"),
            Milestone::AppointmentMade => write!(f, "Appointment Made"),
            Milestone::ContingencySigned => write!(f, "Contingency Signed"),
            Milestone::ContractSigned => write!(f, "Contract Signed"),
            Milestone::Installed => write!(f, "Installed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneDates {
    pub appointment_date: Option<Timestamp>,
    pub contingency_date: Option<Timestamp>,
    pub contract_date: Option<Timestamp>,
    pub install_date: Option<Timestamp>,
    pub loss_date: Option<Timestamp>,
}
impl Index<Milestone> for MilestoneDates {
    type Output = Option<Timestamp>;

    fn index(&self, stage: Milestone) -> &Self::Output {
        static NONE: Option<Timestamp> = None;

        match stage {
            Milestone::LeadAcquired => &NONE,
            Milestone::AppointmentMade => &self.appointment_date,
            Milestone::ContingencySigned => &self.contingency_date,
            Milestone::ContractSigned => &self.contract_date,
            Milestone::Installed => &self.install_date,
        }
    }
}
impl MilestoneDates {
    pub fn timestamps_up_to(&self, stage: Milestone) -> Vec<Option<Timestamp>> {
        Milestone::ordered_iter().take_while(|&s| s <= stage).map(move |s| self[s]).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub jnid: String,
    pub milestone_dates: MilestoneDates,
    pub sales_rep: Option<String>,
	pub insurance_checkbox: bool,
    pub insurance_claim_number: Option<String>,
	pub insurance_company_name: Option<String>,
    pub job_number: Option<String>,
    pub customer_name: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobKind {
    InsuranceWithContingency,
    InsuranceWithoutContingency,
    Retail,
}
impl JobKind {
    pub const NUM_VARIANTS: usize = 3;

    pub const fn into_int(self) -> usize {
        match self {
            JobKind::InsuranceWithContingency => 0,
            JobKind::InsuranceWithoutContingency => 1,
            JobKind::Retail => 2,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AnalyzedJob {
    pub job: Job,
    /// The kind of job that we have. This may not be totally accurate if the
    /// job is not settled.
    pub kind: JobKind,
    /// The dates at which all of the milestones of the job were reached. These
    /// dates must be monotonically increasing. The length is equal to one more
    /// the index of the last milestone reached (e.g. a length of 1 means that
    /// only the first milestone was reached), and None indicates the earliest
    /// possible time which is still in order.
    pub timestamps: Vec<Option<Timestamp>>,
    /// The date at which the job was lost.
    pub loss_timestamp: Option<Timestamp>,
}

impl AnalyzedJob {
    pub fn is_settled(&self) -> bool {
        self.loss_timestamp.is_some() || self.timestamps.len() == Milestone::NUM_VARIANTS
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobAnalysisError {
    #[error(
        "This job has signed a contingency form, but is not an insurance job."
    )]
    ContingencyWithoutInsurance,
	#[error("This job's insurance checkbox isn't checked, but it has an insurance company name and/or claim number.")]
	InconsistentInsuranceInfo,
    #[error("The date for {} does not follow previous dates.", .0.map(|stage| stage.to_string()).unwrap_or("Job Lost".to_owned()))]
    OutOfOrderDates(Option<Milestone>),
    #[error("This job has skipped the date for the milestone {0:?}.")]
    SkippedDates(Milestone),
}

impl TryFrom<Job> for AnalyzedJob {
	type Error = (Job, JobAnalysisError);

	fn try_from(job: Job) -> Result<Self, (Job, JobAnalysisError)> {
        // ensure that the milestone dates make chronological sense

        let mut previous_date = None;
        let mut current_milestone = Milestone::LeadAcquired;
        let mut in_progress = true; // whether retracing of the job's history is still in progress
        let mut kind = if job.insurance_checkbox {
            JobKind::InsuranceWithContingency
        } else {
			// make sure that there isn't inconsistent insurance information
			if job.insurance_company_name.is_some() || job.insurance_claim_number.is_some() {
				return Err((job, JobAnalysisError::InconsistentInsuranceInfo));
			}

            JobKind::Retail
        };
        for milestone in Milestone::ordered_iter().skip(1) {
            let date = job.milestone_dates[milestone];

            if in_progress {
                if let Some(date) = date {
                    // this milestone happened, so update the current milestone accordingly
                    current_milestone = milestone;

                    // update the job kind if necessary
                    if milestone == Milestone::ContingencySigned
                        && job.insurance_claim_number.is_none()
                    {
                        return Err((job, JobAnalysisError::ContingencyWithoutInsurance));
                    }
                    if milestone == Milestone::ContractSigned
                        && job.milestone_dates.contingency_date.is_none()
                        && job.insurance_claim_number.is_some()
                    {
                        kind = JobKind::InsuranceWithoutContingency
                    }

                    // verify that the date is greater than the previous date
                    if let Some(previous_date) = previous_date {
                        if date < previous_date {
                            return Err((job, JobAnalysisError::OutOfOrderDates(Some(milestone))));
                        }
                    }
                    previous_date = Some(date);
                } else {
                    if milestone != Milestone::ContingencySigned {
                        // the job is no longer in progress
                        in_progress = false;
                    }
                    // we make a special exception for the contingency date,
                    // since not all jobs require it
                }
            } else {
                // retracing is no longer in progress, meaning that some
                // previous date was None, so this date must also be None
                if date.is_some() {
                    return Err((job, JobAnalysisError::SkippedDates(current_milestone)));
                }
            }
        }
        if let Some(loss_date) = &job.milestone_dates.loss_date {
            // ensure that the loss date comes after all other dates
            if let Some(previous_date) = &previous_date {
                if loss_date < previous_date {
                    return Err((job, JobAnalysisError::OutOfOrderDates(None)));
                }
            }
        };

        Ok(AnalyzedJob {
            kind,
            timestamps: job.milestone_dates.timestamps_up_to(current_milestone),
            loss_timestamp: job.milestone_dates.loss_date.clone(),
            job,
        })
    }
}

#[derive(Error, Debug)]
pub enum JobFromJsonError {
	#[error("Expected a JSON object, but got {0:?}")]
	NotJsonObject(serde_json::Value),
	#[error("Expected a '{KEY_JNID}' field in the JSON object")]
	JnidNotFound(serde_json::Map<String, serde_json::Value>),
}

impl TryFrom<serde_json::Value> for Job {
    type Error = JobFromJsonError;

    fn try_from(value: serde_json::Value) -> Result<Self, JobFromJsonError> {
        let serde_json::Value::Object(map) = value else {
			return Err(JobFromJsonError::NotJsonObject(value));
        };

        let Some(jnid) = map.get(KEY_JNID).and_then(|val| val.as_str()).map(str::to_owned) else {
			return Err(JobFromJsonError::JnidNotFound(map));
        };

        fn get_owned_nonempty(
            map: &serde_json::Map<String, serde_json::Value>,
            key: &str,
        ) -> Option<String> {
            map.get(key).and_then(|val| val.as_str()).filter(|str| str.len() > 0).map(str::to_owned)
        }

        let sales_rep = get_owned_nonempty(&map, KEY_SALES_REP);
		let insurance_checkbox = map.get(KEY_INSURANCE_CHECKBOX).and_then(|val| val.as_bool()).unwrap_or(false);
		let insurance_company_name = get_owned_nonempty(&map, KEY_INSURANCE_COMPANY_NAME);
        let insurance_claim_number = get_owned_nonempty(&map, KEY_INSURANCE_CLAIM_NUMBER);
        let job_number = get_owned_nonempty(&map, KEY_JOB_NUMBER);
        let customer_name = get_owned_nonempty(&map, KEY_CUSTOMER_NAME);

        // the JobNimbus API sometimes returns a 0 timestamp for a date that has
        // no value, so we want to filter those out as if the value did not
        // exist
        fn get_timestamp_nonzero(
            map: &serde_json::Map<String, serde_json::Value>,
            key: &str,
        ) -> Option<Timestamp> {
            map.get(key)
                .and_then(|value| value.as_i64())
                .filter(|&val| val != 0)
                .and_then(|secs| DateTime::<Utc>::from_timestamp(secs, 0))
        }

        // extract all the milestone dates
        let appointment_date = get_timestamp_nonzero(&map, KEY_APPOINTMENT_DATE);
        let contingency_date = get_timestamp_nonzero(&map, KEY_CONTINGENCY_DATE);
        let contract_date = get_timestamp_nonzero(&map, KEY_CONTRACT_DATE);
        let install_date = get_timestamp_nonzero(&map, KEY_INSTALL_DATE);
        let loss_date = get_timestamp_nonzero(&map, KEY_LOSS_DATE);

        Ok(Job {
            jnid,
            sales_rep,
			insurance_checkbox,
			insurance_company_name,
            insurance_claim_number,
            job_number,
            customer_name,
            milestone_dates: MilestoneDates {
                appointment_date,
                contingency_date,
                contract_date,
                install_date,
                loss_date,
            },
        })
    }
}

#[cfg(test)]
mod test {
    use std::vec;

    use super::*;

    // date-time
    fn dt(seconds: i64) -> Timestamp {
        Timestamp::from_timestamp(seconds, 0).unwrap()
    }

    fn make_job(
        insurance: bool,
        date_1: Option<Timestamp>,
        date_2: Option<Timestamp>,
        date_3: Option<Timestamp>,
        date_4: Option<Timestamp>,
        date_5: Option<Timestamp>,
    ) -> Job {
        Job {
            jnid: "0".to_owned(),
            sales_rep: None,
			insurance_checkbox: insurance,
            insurance_claim_number: if insurance { Some("123".to_owned()) } else { None },
			insurance_company_name: if insurance { Some("Gekko".to_owned()) } else { None },
            job_number: None,
            customer_name: None,
            milestone_dates: MilestoneDates {
                appointment_date: date_1,
                contingency_date: date_2,
                contract_date: date_3,
                install_date: date_4,
                loss_date: date_5,
            },
        }
    }

    #[test]
    fn job_status_retail_without_contingency() {
        let job = make_job(false, Some(dt(1)), None, Some(dt(3)), Some(dt(4)), None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::Retail,
                timestamps: vec![None, Some(dt(1)), None, Some(dt(3)), Some(dt(4))],
                loss_timestamp: None,
            })
        );
    }

    #[test]
    fn job_status_retail_with_contingency() {
        let job = make_job(false, Some(dt(1)), Some(dt(2)), Some(dt(3)), Some(dt(4)), None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Err((job, JobAnalysisError::ContingencyWithoutInsurance))
        );
    }

    #[test]
    fn job_status_insurance_without_contingency() {
        let job = make_job(true, Some(dt(1)), None, Some(dt(3)), Some(dt(4)), None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithoutContingency,
                timestamps: vec![None, Some(dt(1)), None, Some(dt(3)), Some(dt(4))],
                loss_timestamp: None,
            })
        );
    }

    #[test]
    fn job_status_insurance_with_contingency() {
        let job = make_job(true, Some(dt(1)), Some(dt(2)), Some(dt(3)), Some(dt(4)), None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None, Some(dt(1)), Some(dt(2)), Some(dt(3)), Some(dt(4))],
                loss_timestamp: None,
            })
        );
    }

    #[test]
    fn job_status_insurance_at_each_stage() {
        let job = make_job(true, None, None, None, None, None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None],
                loss_timestamp: None,
            })
        );

        let job = make_job(true, Some(dt(1)), None, None, None, None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None, Some(dt(1))],
                loss_timestamp: None,
            })
        );

        let job = make_job(true, Some(dt(1)), Some(dt(2)), None, None, None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None, Some(dt(1)), Some(dt(2))],
                loss_timestamp: None,
            })
        );

        let job = make_job(true, Some(dt(1)), Some(dt(2)), Some(dt(3)), None, None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None, Some(dt(1)), Some(dt(2)), Some(dt(3))],
                loss_timestamp: None,
            })
        );

        let job = make_job(true, Some(dt(1)), Some(dt(2)), Some(dt(3)), Some(dt(4)), None);
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None, Some(dt(1)), Some(dt(2)), Some(dt(3)), Some(dt(4))],
                loss_timestamp: None,
            })
        );

        let job = make_job(true, Some(dt(1)), None, None, None, Some(dt(5)));
        assert_eq!(
            AnalyzedJob::try_from(job.clone()),
            Ok(AnalyzedJob {
                job,
                kind: JobKind::InsuranceWithContingency,
                timestamps: vec![None, Some(dt(1))],
                loss_timestamp: Some(dt(5)),
            })
        );
    }
}