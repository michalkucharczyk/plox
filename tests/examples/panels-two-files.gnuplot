set terminal pngcairo enhanced font 'arial,10' fontscale 3.0 size 7560, 5500
set output './tests/.output/panels-two-files.png'
set linetype 1 lc rgb "red" dt 1 pt 7 lw 2.0 ps 4.0
set linetype 2 lc rgb "blue" dt 1 pt 9 lw 2.0 ps 4.0
set linetype 3 lc rgb "dark-green" dt 1 pt 5 lw 2.0 ps 4.0
set linetype 4 lc rgb "purple" dt 1 pt 13 lw 2.0 ps 4.0
set linetype 5 lc rgb "cyan" dt 1 pt 1 lw 2.0 ps 4.0
set linetype 6 lc rgb "goldenrod" dt 1 pt 3 lw 2.0 ps 4.0
set linetype 7 lc rgb "brown" dt 1 pt 6 lw 2.0 ps 4.0
set linetype 8 lc rgb "olive" dt 1 pt 2 lw 2.0 ps 4.0
set linetype 9 lc rgb "navy" dt 1 pt 8 lw 2.0 ps 4.0
set linetype 10 lc rgb "violet" dt 1 pt 4 lw 2.0 ps 4.0
set linetype 11 lc rgb "coral" dt 1 pt 12 lw 2.0 ps 4.0
set linetype 12 lc rgb "salmon" dt 1 pt 7 lw 2.0 ps 4.0
set linetype 13 lc rgb "steelblue" dt 1 pt 9 lw 2.0 ps 4.0
set linetype 14 lc rgb "dark-magenta" dt 1 pt 5 lw 2.0 ps 4.0
set linetype 15 lc rgb "dark-cyan" dt 1 pt 13 lw 2.0 ps 4.0
set linetype 16 lc rgb "dark-yellow" dt 1 pt 1 lw 2.0 ps 4.0
set linetype 17 lc rgb "dark-turquoise" dt 1 pt 3 lw 2.0 ps 4.0
set linetype 18 lc rgb "yellow" dt 1 pt 6 lw 2.0 ps 4.0
set linetype 19 lc rgb "black" dt 1 pt 2 lw 2.0 ps 4.0
set linetype 20 lc rgb "magenta" dt 1 pt 8 lw 2.0 ps 4.0
set datafile separator ','
set xdata time
set timefmt '%Y-%m-%dT%H:%M:%S'
set format x '%H:%M:%S'
set mxtics 10
set grid xtics mxtics
set grid ytics mytics
set ytics nomirror
set key noenhanced
set multiplot
set lmargin at screen 0.035
set rmargin at screen 0.975
combine_datetime(date_col,time_col) = strcol(date_col) . 'T' . strcol(time_col)
set origin 0.0,0
set size 1.0,0.16166666666666665
unset label
set label '[default-other]' at graph -0.03,0.5 rotate by 90 center font"arial bold,10"
unset logscale y
set y2tics nomirror
set my2tics 10
set xrange ["2020-01-01T00:00:00":"2020-01-01T00:16:34"]
csv_data_file_0000 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default-other.log_1745784327__foo_module__value_1_SOME_EVENT.csv'
csv_data_file_0001 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default-other.log_1745784327__foo_module__value_1_SOME_EVENT.csv'
plot \
   csv_data_file_0000 using (combine_datetime('date','time')):'count' with lines axes x1y1 title 'count of foo_module SOME_EVENT (default-other)', \
   csv_data_file_0001 using (combine_datetime('date','time')):'value' with points ps 2 axes x1y2 title 'presence of foo_module SOME_EVENT (default-other) | y2'
unset y2tics
unset my2tics
set origin 0.0,0.16166666666666665
set size 1.0,0.16166666666666665
unset label
set label '[default]' at graph -0.03,0.5 rotate by 90 center font"arial bold,10"
unset logscale y
set y2tics nomirror
set my2tics 10
set xrange ["2020-01-01T00:00:00":"2020-01-01T00:16:34"]
csv_data_file_0000 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default.log_1745784327__foo_module__value_1_SOME_EVENT.csv'
csv_data_file_0001 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default.log_1745784327__foo_module__value_1_SOME_EVENT.csv'
plot \
   csv_data_file_0000 using (combine_datetime('date','time')):'count' with lines axes x1y1 title 'count of foo_module SOME_EVENT (default)', \
   csv_data_file_0001 using (combine_datetime('date','time')):'value' with points ps 2 axes x1y2 title 'presence of foo_module SOME_EVENT (default) | y2'
unset y2tics
unset my2tics
set origin 0.0,0.3233333333333333
set size 1.0,0.16166666666666665
unset label
set label '[default-other]' at graph -0.03,0.5 rotate by 90 center font"arial bold,10"
unset logscale y
set xrange ["2020-01-01T00:00:00":"2020-01-01T00:16:34"]
csv_data_file_0000 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default-other.log_1745784327__x_module__%5Cbx01%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
csv_data_file_0001 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default-other.log_1745784327__x_module__%5Cbx02%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
csv_data_file_0002 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default-other.log_1745784327__x_module__%5Cbx03%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
plot \
   csv_data_file_0000 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of x_module x01 (default-other)', \
   csv_data_file_0001 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of x_module x02 (default-other)', \
   csv_data_file_0002 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of x_module x03 (default-other)'
unset y2tics
unset my2tics
set origin 0.0,0.485
set size 1.0,0.16166666666666665
unset label
set label '[default]' at graph -0.03,0.5 rotate by 90 center font"arial bold,10"
unset logscale y
set xrange ["2020-01-01T00:00:00":"2020-01-01T00:16:34"]
csv_data_file_0000 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default.log_1745784327__x_module__%5Cbx01%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
csv_data_file_0001 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default.log_1745784327__x_module__%5Cbx02%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
csv_data_file_0002 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default.log_1745784327__x_module__%5Cbx03%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
plot \
   csv_data_file_0000 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of x_module x01 (default)', \
   csv_data_file_0001 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of x_module x02 (default)', \
   csv_data_file_0002 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of x_module x03 (default)'
unset y2tics
unset my2tics
set origin 0.0,0.6466666666666666
set size 1.0,0.16166666666666665
unset label
set label '[default-other]' at graph -0.03,0.5 rotate by 90 center font"arial bold,10"
unset logscale y
set xrange ["2020-01-01T00:00:00":"2020-01-01T00:16:34"]
csv_data_file_0000 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default-other.log_1745784327__om_module__%5Cbx%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
plot \
   csv_data_file_0000 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of om_module x (default-other)'
unset y2tics
unset my2tics
set origin 0.0,0.8083333333333332
set size 1.0,0.16166666666666665
unset label
set label '[default]' at graph -0.03,0.5 rotate by 90 center font"arial bold,10"
unset logscale y
set xrange ["2020-01-01T00:00:00":"2020-01-01T00:16:34"]
csv_data_file_0000 = '/home/miszka/parity/graph-tool/plox/tests/examples/.plox/default.log_1745784327__om_module__%5Cbx%3D%28%5B%5Cd%5C.%5D%2B%29%28%5Cw%2B%29%3F.csv'
plot \
   csv_data_file_0000 using (combine_datetime('date','time')):'value' with lines axes x1y1 title 'value of om_module x (default)'
unset y2tics
unset my2tics
unset multiplot
