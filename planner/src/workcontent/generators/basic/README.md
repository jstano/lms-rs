### basic_standards_generator
* call basic_standards_processor
* save work_contents
* planned_shift_creator.create_planned_shifts(work_contents)
* save planned_shifts

### basic_standards_processor
* for dates in period
  * for each day
    * for each shift in standard_set
      * totalWorkMinutesForStandardsCalculator.totalWorkMinutes
        * basic_work_content_creator.create_work_content(day, shift)
        * save work_content




